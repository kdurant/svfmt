/*=============================================================================
# FileName    :   spi_master_core.sv
# Author      :   author
# Email       :   email@email.com
# Description :   SPI Master Core with Quad (QSPI) support
#                 AXI-Stream protocol (per transfer = 1 byte):
#                   tdest[0]: 0=1-bit mode, 1=quad (4-bit) mode
#                   tdest[1]: 0=TX (drive), 1=RX (capture)
#                   tlast:    1=end of SPI frame (de-assert CS)
#                 tdata[7:0]: data byte to transmit (ignored in RX mode)
#                 SPI Mode 0: data driven on falling edge, captured on rising edge
# Version     :   1.0
# LastChange  :   2026-07-28
# ChangeLog   :
=============================================================================*/

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

logic [31:0] byte_cnt;                  // bytes remaining for data phase
logic        axis_m_tvalid;
logic [7:0]  axis_m_tdata;
logic [3:0]  axis_m_tdest;
logic        axis_m_tlast;

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
    end
    else
    begin

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

        // ---- RDID (0x9F) ----
        ST_RDID_CMD :
            if(axis_handshake)
                state_nxt = ST_RDID_DATA;
        ST_RDID_DATA :
            if(axis_handshake)
            begin
                if(byte_cnt   <= 1)
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
                else                    // WIP = 0 → done (internal poll)
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

    endcase
end

endmodule
