/* =============================================================================
# FileName    :	flash_manage.sv
# Author      :	author
# Email       :	email@email.com
# Description :	主要功能：
                Flash 命令
                | 分类 | Command                    | Opcode           |
                | -- | -------------------------- | ---------------- | -
                | 识别 | Read ID                    | `0x9F`           |
                | 状态 | Read Status                | `0x05`           |
                | 控制 | Write Enable               | `0x06`           |
                | 擦除 | Sector Erase               | `0x20`           |
                | 写入 | Page Program               | `0x02`           |
                | 读取 | Read Data                  | `0x03`           |
                - 0x03命令，可以读取任意长度的数据
                - sector: 4K Byte

                页边界处理
                Busy 状态管理
                Flash 错误检测
                数据收发协调
# Version     :	1.0
# LastChange  :	2026-07-28 10:07:32
# ChangeLog   :	
============================================================================= */

`timescale  1 ns/1 ps

module flash_manage
(
    input                clk,
    input                rst,
    /*port*/
    if_spi_flash.slave   spi_flash,

    if_axi_stream.slave  axis_wr_in,
    if_axi_stream.master axis_flash_wr,

    if_axi_stream.slave  axis_flash_rd,
    if_axi_stream.master axis_rd_out
);

// ---------------------------------------------------------------------------
// Command encoding (user-facing, use the flash instruction opcode directly)
// ---------------------------------------------------------------------------
localparam CMD_RDID = 8'h9F;            // Read ID
localparam CMD_RDSR = 8'h05;            // Read Status
localparam CMD_WREN = 8'h06;            // Write Enable
localparam CMD_SE   = 8'h20;            // Sector Erase 4KB
localparam CMD_PP   = 8'h02;            // Page Program
localparam CMD_READ = 8'h03;            // Read Data

// ---------------------------------------------------------------------------
// Opcodes
// ---------------------------------------------------------------------------
localparam OP_WREN = 8'h06;
localparam OP_WRDI = 8'h04;
localparam OP_RDSR = 8'h05;
localparam OP_WRSR = 8'h01;
localparam OP_SE   = 8'h20;
localparam OP_PP   = 8'h02;
localparam OP_READ = 8'h03;
localparam OP_RDID = 8'h9F;

// ---------------------------------------------------------------------------
// AXI-Stream tdest encoding for SPI master core
// ---------------------------------------------------------------------------
localparam TDEST_1BIT_TX = 2'b00;       // [1:0] → 1-bit TX
localparam TDEST_1BIT_RX = 2'b10;       // [1:0] → 1-bit RX

// ---------------------------------------------------------------------------
// FSM
// ---------------------------------------------------------------------------
typedef enum logic [5:0] {
    ST_IDLE,
    // --- WREN sub-sequence ---
    ST_WREN_CMD,
    ST_WREN_DONE,
    // --- RDID (0x9F) sub-sequence ---
    ST_RDID_CMD,
    ST_RDID_DATA,
    ST_RDID_DONE,
    // --- Busy Poll / RDSR sub-sequence (internal) ---
    ST_POLL_CMD,                        // send RDSR opcode (TX)
    ST_POLL_RX,                         // dummy byte to capture status (RX)
    ST_POLL_WAIT,                       // check poll_status[0]
    // --- SE (0x20) sub-sequence ---
    ST_SE_CMD,
    ST_SE_ADDR_B2,
    ST_SE_ADDR_B1,
    ST_SE_ADDR_B0,
    ST_SE_DONE,
    // --- PP (0x02) sub-sequence ---
    ST_PP_CMD,
    ST_PP_ADDR_B2,
    ST_PP_ADDR_B1,
    ST_PP_ADDR_B0,
    ST_PP_DATA_RX,
    ST_PP_DATA_TX,
    ST_PP_DONE,
    // --- READ (0x03) sub-sequence ---
    ST_READ_CMD,
    ST_READ_ADDR_B2,
    ST_READ_ADDR_B1,
    ST_READ_ADDR_B0,
    ST_READ_DATA,
    ST_READ_DONE,
    // --- WRSR (0x01) sub-sequence ---
    ST_WRSR_CMD,
    ST_WRSR_DATA_RX,
    ST_WRSR_DATA_TX,
    ST_WRSR_DONE
} state_t;

state_t state, state_nxt;

// ---------------------------------------------------------------------------
// Registers
// ---------------------------------------------------------------------------
logic [31:0] byte_cnt;                  // bytes remaining for data phase
logic        axis_m_tvalid;
logic [7:0]  axis_m_tdata;
logic [3:0]  axis_m_tdest;
logic        axis_m_tlast;

logic       axis_s_tready;
logic [7:0] s_data_buf;                 // buffered data from axis_wr_in

// Read-data pipeline (1-deep)
logic       rd_valid;
logic [7:0] rd_data;
logic       rd_last;

// Busy-poll registers (internal RDSR → check WIP)
logic [7:0] poll_status;
logic       poll_done;
logic       is_user_rdsr;               // 1 = CMD_RDSR from user, forward status to axis_rd_out

wire rd_consume = rd_valid && axis_rd_out.tready;
wire rd_accept  = axis_flash_rd.tvalid && (!rd_valid || rd_consume);

// ---------------------------------------------------------------------------
// AXI-Stream master output to SPI master
// ---------------------------------------------------------------------------
assign axis_flash_wr.tvalid = axis_m_tvalid;
assign axis_flash_wr.tdata  = axis_m_tdata;
assign axis_flash_wr.tdest  = axis_m_tdest;
assign axis_flash_wr.tlast  = axis_m_tlast;
assign axis_flash_wr.tlen   = spi_flash.len;

// Command ready: FSM is idle and can accept next command
assign spi_flash.cmd_rdy = (state == ST_IDLE);

// AXI-Stream slave: tready to accept write data
assign axis_wr_in.tready = axis_s_tready;

// Read-data pipeline: accept from SPI master, output unified
wire in_poll                = (state == ST_POLL_CMD || state == ST_POLL_RX || state == ST_POLL_WAIT);
assign axis_flash_rd.tready = in_poll ? 1'b1 :(!rd_valid || rd_consume);
assign axis_rd_out.tvalid   = rd_valid;
assign axis_rd_out.tdata    = rd_data;
assign axis_rd_out.tlast    = rd_last;
assign axis_rd_out.tdest    = 0;
assign axis_rd_out.tlen     = 0;

// ---------------------------------------------------------------------------
// Sequential
// ---------------------------------------------------------------------------
always_ff @(posedge clk or posedge rst)
begin
    if(rst)
    begin
        state         <= ST_IDLE;
        byte_cnt      <= 0;
        axis_m_tvalid <= 0;
        axis_m_tdata  <= 0;
        axis_m_tdest  <= 0;
        axis_m_tlast  <= 0;
        axis_s_tready <= 0;
        s_data_buf    <= 0;
        rd_valid      <= 0;
        rd_data       <= 0;
        rd_last       <= 0;
        poll_status   <= 0;
        poll_done     <= 0;
        is_user_rdsr  <= 0;
    end
    else
    begin
        state         <= state_nxt;
        axis_m_tvalid <= 0;             // default: pulse
        axis_s_tready <= 0;

        // Read-data pipeline (external output; skip during busy poll)
        if(rd_accept && !(state == ST_POLL_CMD || state == ST_POLL_RX || state == ST_POLL_WAIT))
        begin
            rd_valid <= 1;
            rd_data  <= axis_flash_rd.tdata;
            rd_last  <= axis_flash_rd.tlast;
        end
        else if(rd_consume)
        begin
            rd_valid <= 0;
        end

        // Busy-poll data capture (internal, when in ST_POLL_WAIT)
        if(state == ST_POLL_WAIT)
        begin
            poll_done <= 0;
            if(rd_accept)
            begin
                poll_status <= axis_flash_rd.tdata;
                poll_done   <= 1;
            end
        end

        // User RDSR: forward captured status to external pipeline (1 cycle after poll_done)
        if(state == ST_POLL_WAIT && poll_done && is_user_rdsr)
        begin
            rd_valid     <= 1;
            rd_data      <= poll_status;
            rd_last      <= 1;
            is_user_rdsr <= 0;
        end

        // byte_cnt management (use raw handshake since axis_handshake wire is later)
        case(state)
            ST_RDID_CMD     :
                if(axis_m_tvalid && axis_flash_wr.tready)
                    byte_cnt <= spi_flash.len;
            ST_READ_ADDR_B0 :
                if(axis_m_tvalid && axis_flash_wr.tready)
                    byte_cnt <= spi_flash.len;
            ST_PP_ADDR_B0   :
                if(axis_m_tvalid && axis_flash_wr.tready)
                    byte_cnt <= spi_flash.len;
            ST_RDID_DATA    :
                if(axis_m_tvalid && axis_flash_wr.tready && byte_cnt > 1)
                    byte_cnt <= byte_cnt - 1;
            ST_READ_DATA    :
                if(axis_m_tvalid && axis_flash_wr.tready && byte_cnt > 1)
                    byte_cnt <= byte_cnt - 1;
            ST_PP_DATA_TX   :
                if(axis_m_tvalid && axis_flash_wr.tready && byte_cnt > 1)
                    byte_cnt <= byte_cnt - 1;
        endcase

        case(state)
            // ---------------------------------------------------------------
            ST_IDLE :
            begin
                if(spi_flash.cmd_vld)
                begin
                    is_user_rdsr <= (spi_flash.cmd == CMD_RDSR);
                end
            end

            // ===============================================================
            // WREN sub-sequence  (also used as preamble by PP, SE, WRSR)
            // ===============================================================
            ST_WREN_CMD :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= OP_WREN;
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 1;
                end
            end

            ST_WREN_DONE :
            begin
            end

            // ===============================================================
            // RDID (0x9F) sub-sequence  command → data out
            // ===============================================================
            ST_RDID_CMD :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= OP_RDID;
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_RDID_DATA :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= 8'h00;
                    axis_m_tdest  <= TDEST_1BIT_RX;
                    axis_m_tlast  <= (byte_cnt <= 1);
                end
            end

            ST_RDID_DONE :
            begin
            end

            // ===============================================================
            // RDSR / Busy Poll — send RDSR opcode + dummy RX, check status
            // ===============================================================
            ST_POLL_CMD :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= OP_RDSR;
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;               // keep CS low for status byte
                end
            end

            ST_POLL_RX :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= 8'h00;
                    axis_m_tdest  <= TDEST_1BIT_RX;   // capture status on MISO
                    axis_m_tlast  <= 1;
                end
            end

            ST_POLL_WAIT :
            begin
            end

            // ===============================================================
            // SE (0x20) sub-sequence  (1-bit address, opcode, tlast)
            // ===============================================================
            ST_SE_CMD :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= OP_SE;
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_SE_ADDR_B2 :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= spi_flash.addr[23:16];
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_SE_ADDR_B1 :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= spi_flash.addr[15:8];
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_SE_ADDR_B0 :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= spi_flash.addr[7:0];
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 1;
                end
            end

            ST_SE_DONE :
            begin
            end

            // ===============================================================
            // PP (0x02) sub-sequence  WREN → opcode → addr3 → data in
            // ===============================================================
            ST_PP_CMD :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= OP_PP;
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_PP_ADDR_B2 :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= spi_flash.addr[23:16];
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_PP_ADDR_B1 :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= spi_flash.addr[15:8];
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_PP_ADDR_B0 :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= spi_flash.addr[7:0];
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_PP_DATA_RX :
            begin
                axis_s_tready <= 1;
                if(axis_wr_in.tvalid)
                begin
                    s_data_buf <= axis_wr_in.tdata;
                end
            end

            ST_PP_DATA_TX :
            begin
                axis_m_tvalid <= 1;
                axis_m_tdata  <= s_data_buf;
                axis_m_tdest  <= TDEST_1BIT_TX;
                axis_m_tlast  <= (byte_cnt <= 1);
            end

            ST_PP_DONE :
            begin
            end

            // ===============================================================
            // READ (0x03) sub-sequence  opcode → addr3 → data out
            // ===============================================================
            ST_READ_CMD :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= OP_READ;
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_READ_ADDR_B2 :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= spi_flash.addr[23:16];
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_READ_ADDR_B1 :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= spi_flash.addr[15:8];
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_READ_ADDR_B0 :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= spi_flash.addr[7:0];
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_READ_DATA :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= 8'h00;
                    axis_m_tdest  <= TDEST_1BIT_RX;
                    axis_m_tlast  <= (byte_cnt <= 1);
                end
            end

            ST_READ_DONE :
            begin
            end

            // ===============================================================
            // WRSR (0x01) sub-sequence
            // ===============================================================
            ST_WRSR_CMD :
            begin
                if(axis_flash_wr.tready)
                begin
                    axis_m_tvalid <= 1;
                    axis_m_tdata  <= OP_WRSR;
                    axis_m_tdest  <= TDEST_1BIT_TX;
                    axis_m_tlast  <= 0;
                end
            end

            ST_WRSR_DATA_RX :
            begin
                axis_s_tready <= 1;
                if(axis_wr_in.tvalid)
                begin
                    s_data_buf <= axis_wr_in.tdata;
                end
            end

            ST_WRSR_DATA_TX :
            begin
                axis_m_tvalid <= 1;
                axis_m_tdata  <= s_data_buf;
                axis_m_tdest  <= TDEST_1BIT_TX;
                axis_m_tlast  <= 1;
            end

            ST_WRSR_DONE :
            begin
            end
        endcase
    end
end

// ---------------------------------------------------------------------------
// Next-state logic
// ---------------------------------------------------------------------------
wire axis_handshake = axis_m_tvalid && axis_flash_wr.tready;

always_comb
begin
    state_nxt = state;
    case(state)
        // ===========================================================
        ST_IDLE :
        begin
            if(spi_flash.cmd_vld)
            begin
                case(spi_flash.cmd)
                    CMD_RDID : state_nxt = ST_RDID_CMD;
                    CMD_RDSR : state_nxt = ST_POLL_CMD;  // reuse busy-poll path (2-byte RDSR)
                    CMD_WREN : state_nxt = ST_WREN_CMD;
                    CMD_SE   : state_nxt = ST_SE_CMD;
                    CMD_PP   : state_nxt = ST_PP_CMD;
                    CMD_READ : state_nxt = ST_READ_CMD;
                    default  : state_nxt = ST_IDLE;
                endcase
            end
        end

        // ---- WREN sub-sequence ----
        ST_WREN_CMD :
        begin
            if(axis_handshake)
            begin
                state_nxt = ST_WREN_DONE;
            end
        end
        ST_WREN_DONE :
        begin
            state_nxt = ST_IDLE;
        end

        // ---- RDID (0x9F) ----
        ST_RDID_CMD :
            if(axis_handshake)
                state_nxt = ST_RDID_DATA;
        ST_RDID_DATA :
            if(axis_handshake)
            begin
                if(byte_cnt <= 1)
                    state_nxt = ST_RDID_DONE;
                else
                    state_nxt = ST_RDID_DATA;
            end
        ST_RDID_DONE : state_nxt = ST_IDLE;

        // ---- RDSR (0x05, now via busy-poll sub-sequence) ----
        // (route CMD_RDSR → ST_POLL_CMD → ST_POLL_RX → ST_POLL_WAIT → ST_IDLE)

        // ---- Busy Poll / RDSR ----
        ST_POLL_CMD :
            if(axis_handshake)
                state_nxt = ST_POLL_RX;
        ST_POLL_RX :
            if(axis_handshake)
                state_nxt = ST_POLL_WAIT;
        ST_POLL_WAIT :
            if(poll_done)
            begin
                if(is_user_rdsr)
                    state_nxt = ST_IDLE;  // user RDSR: exit after one poll
                else if(poll_status[0])   // WIP = 1 → still busy (internal poll)
                    state_nxt = ST_POLL_CMD;
                else  // WIP = 0 → done (internal poll)
                    state_nxt = ST_IDLE;
            end

        // ---- SE (0x20) ----
        ST_SE_CMD :
            if(axis_handshake)
                state_nxt = ST_SE_ADDR_B2;
        ST_SE_ADDR_B2 :
            if(axis_handshake)
                state_nxt = ST_SE_ADDR_B1;
        ST_SE_ADDR_B1 :
            if(axis_handshake)
                state_nxt = ST_SE_ADDR_B0;
        ST_SE_ADDR_B0 :
            if(axis_handshake)
                state_nxt = ST_SE_DONE;
        ST_SE_DONE : state_nxt = ST_IDLE;  // no auto-poll; upper layer controls

        // ---- PP (0x02) ----
        ST_PP_CMD :
            if(axis_handshake)
                state_nxt = ST_PP_ADDR_B2;
        ST_PP_ADDR_B2 :
            if(axis_handshake)
                state_nxt = ST_PP_ADDR_B1;
        ST_PP_ADDR_B1 :
            if(axis_handshake)
                state_nxt = ST_PP_ADDR_B0;
        ST_PP_ADDR_B0 :
            if(axis_handshake)
            begin
                state_nxt = ST_PP_DATA_RX;
            end
        ST_PP_DATA_RX :
            if(axis_wr_in.tvalid)
            begin
                state_nxt = ST_PP_DATA_TX;
            end
        ST_PP_DATA_TX :
            if(axis_handshake)
            begin
                if(byte_cnt <= 1)
                    state_nxt = ST_PP_DONE;
                else
                    state_nxt = ST_PP_DATA_RX;
            end
        ST_PP_DONE : state_nxt = ST_IDLE;  // no auto-poll; upper layer controls

        // ---- READ (0x03) ----
        ST_READ_CMD :
            if(axis_handshake)
                state_nxt = ST_READ_ADDR_B2;
        ST_READ_ADDR_B2 :
            if(axis_handshake)
                state_nxt = ST_READ_ADDR_B1;
        ST_READ_ADDR_B1 :
            if(axis_handshake)
                state_nxt = ST_READ_ADDR_B0;
        ST_READ_ADDR_B0 :
            if(axis_handshake)
            begin
                state_nxt = ST_READ_DATA;
            end
        ST_READ_DATA :
            if(axis_handshake)
            begin
                if(byte_cnt <= 1)
                    state_nxt = ST_READ_DONE;
                else
                    state_nxt = ST_READ_DATA;
            end
        ST_READ_DONE : state_nxt = ST_IDLE;

        // ---- WRSR (0x01) ----
        ST_WRSR_CMD :
            if(axis_handshake)
            begin
                state_nxt = ST_WRSR_DATA_RX;
            end
        ST_WRSR_DATA_RX :
            if(axis_wr_in.tvalid)
            begin
                state_nxt = ST_WRSR_DATA_TX;
            end
        ST_WRSR_DATA_TX :
            if(axis_handshake)
            begin
                state_nxt = ST_WRSR_DONE;
            end
        ST_WRSR_DONE : state_nxt = ST_IDLE;

        default : state_nxt = ST_IDLE;
    endcase
end

// synthesis translate_off
logic [195:0] cmd_STRING;
always_comb
begin
    case(spi_flash.cmd)
        CMD_RDID : cmd_STRING = "Read ID(0x9f)";
        CMD_RDSR : cmd_STRING = "Read Status(0x05)";
        CMD_WREN : cmd_STRING = "Write Enable(0x06)";
        CMD_SE   : cmd_STRING = "Sector Erase(0x20)";
        CMD_PP   : cmd_STRING = "Page Program (0x02)";
        CMD_READ : cmd_STRING = "Read Data(0x03)";
        default  : cmd_STRING = "NONE";
    endcase
end
// synthesis translate_on

// synthesis translate_off
always_ff @(posedge clk)
begin
    if(state == ST_WREN_CMD && axis_m_tvalid && axis_flash_wr.tready)
        $display("[%0t] flash_manage: WREN (0x06) sent to SPI master", $time);
    if(state == ST_PP_CMD && axis_m_tvalid && axis_flash_wr.tready)
        $display("[%0t] flash_manage: PP (0x02) sent to SPI master", $time);
end
// synthesis translate_on

endmodule
