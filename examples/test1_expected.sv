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

module spi_master_core #
(
    parameter int SYS_FREQ = 125_000_000,
    parameter int FREQ     = 1_000_000
)
(
    input  wire          clk,
    input  wire          rst,

    if_axi_stream.slave  axis_slave,
    if_axi_stream.master axis_master,
    output reg [15:00]   data_out = 0,

    input      [7:0]     gps_m_axis_tdata,
    output reg [31:0]    gps_week,
    output reg [63:0]    gps_second,

    if_gmii              gmii_i,
    input      [08:00]   uart_rxd,
    output     [08:00]   uart_txd,
    if_ad7928            ad7928,
    if_ad5328            ad5328,

    inout  wire          mosi,          // SIO0
    inout  wire          miso,          // SIO1
    inout  wire          wp,            // SIO2
    inout  wire          sio3,          // SIO3
    output logic         sclk,
    output logic         scs
);

always_ff @(posedge clk or posedge rst)
begin
    if(rst)
        bit_cnt <= 0;
    else if(state == ST_HIGH && cnt == FREQ_CNT - 1)
    begin
        // At the end of HIGH phase (falling edge about to happen)
        if(byte_done)
            bit_cnt <= 0;
        else
            bit_cnt <= bit_cnt + 1;
    end
    else if(state == ST_NEXT || state == ST_WAIT || state == ST_HOLD || state == ST_IDLE)
        bit_cnt <= 0;
end

if_axi_stream #(.DATA_WIDTH(8)) fifo_if();

assign dev_info.fpga_version     = `PROJECT_VERSION_FULL;
assign dev_info.fpga_dna[127:57] = 0;
assign dev_info.device_sn        = "1122334455667788";
assign dev_info.device_type      = 1;

logic rst;
logic clk_40m;                          // 20Mbps, 使用40Mhz时钟编码发送
logic clk_100m;

typedef enum logic [5:0] {
    ST_IDLE,
    // --- WREN sub-sequence ---
    ST_WREN_CMD,
    ST_WREN_DONE,
    ST_WRSR_DONE
} state_t;

localparam CMD_READ_SYS_STATUS = 16'h0100;
localparam CMD_UP_INTERVAL     = 16'h0101;
localparam CMD_SETTING_MODE    = 16'h0200;
localparam CMD_ALIVE_INTERVAL  = 16'h0201;
localparam CMD_PMT_GAIN_MANUAL = 16'h0210;  // CMD_PMT_GAIN_MANUAL
localparam CMD_LD_VOLTAGE      = 16'h0211;
localparam CMD_RSSI_RANGE_MIN  = 16'h0220;  // CMD_RSSI_RANGE_MAX
localparam CMD_RSSI_RANGE_MAX  = 16'h0221;  // CMD_RSSI_RANGE_MIN
localparam CMD_FLASH_ADDR      = 16'h0600;
localparam CMD_FLASH_READ      = 16'h0601;
localparam CMD_FLASH_WRITE     = 16'h0602;

localparam HEAD_LEN                   = 26;
logic [HEAD_LEN-1:00][07:00] axis_reg = 0;
assign axis_slave.tready              = 1'b1;

always_ff @(posedge clk)
begin
    axis_reg <= {axis_reg[HEAD_LEN-2:0], axis_slave.tdata};
end

logic [15:00] tlast_reg = 0;
always_ff @(posedge clk)
begin
    tlast_reg <= {tlast_reg[14:0], axis_slave.tlast};
end

assign spi_flash.cmd_rdy = (state == ST_IDLE);

// synthesis translate_off
reg [127:0] cs_STRING;
always @(*)
begin
    case(1'b1)
        cs[IDLE]         : cs_STRING = "IDLE";
        cs[PASS_THROUGH] : cs_STRING = "PASS_THROUGH";
        cs[PEER_INFO]    : cs_STRING = "PEER_INFO";
        cs[OVER]         : cs_STRING = "OVER";
        default          : cs_STRING = "XXXX";
    endcase
end
// synthesis translate_on

always @(posedge clk)
begin
    if(cs[SEARCH] & ns[OVER])
        edge_pos <= stream_cnt + 1;

    case(state)
        ST_RDID_CMD :
        begin
            if(axis_m_tvalid && axis_flash_wr.tready)
                byte_cnt <= spi_flash.len;
        end
        ST_READ_ADDR_B0 :
            if(axis_m_tvalid && axis_flash_wr.tready)
                byte_cnt <= spi_flash.len;
        ST_PP_ADDR_B0 :
            if(axis_m_tvalid && axis_flash_wr.tready)
                byte_cnt <= spi_flash.len;
    endcase
end

ad5314_top ad5314_top_Ex01
(
    .clk       (  clk_125m                      ),
    .dac_set   (  dac_set                       ),
    .ch0_value (  dev_status.ld_voltage[09:00]  ),
    .ch1_value (  dev_status.pmt_gain           ),
    .ch2_value (  16'd555                       ),
    .ch3_value (  16'd777                       ),
    .ldac_n    (                                ),
    .sync_n    (  adc5314_sync                  ),
    .sclk      (  adc5314_sclk                  )
);

datamover_wrap #
(
    .BASE_ADDR     (  32'h3C000000              ),
    .WIDTH         (  32                        )
)
datamover_wrapEx01
(
    .clk           (  clk_125m                  ),
    .start         (  start_dma_read            ),
    .sectors       (  sata_ctl.app_tot_sec_cnt  ),
    .s_axis_tvalid (  axis_dma_if.tvalid        ),
    .s_axis_tready (  axis_dma_if.tready        ),
    .s_axis_tdata  (  axis_dma_if.tdata         )
);

parameter[3:0] USE_ILA_FOR_INST = 4'b0000;
genvar i;
generate
    for(i = 0; i < LANES; i = i + 1)
    begin
        truncate #
        (
            .USE_ILA       (  USE_ILA_FOR_INST[i]          )
        )
        truncateEx01
        (
            .clk           (  clk                          ),
            .trg           (  trg                          ),
            .first_pos     (  capture_set.first_pos >> 3   ),
            .first_len     (  capture_set.first_len >> 3   ),
            .second_pos    (  capture_set.second_pos >> 3  ),
            .second_len    (  capture_set.second_len >> 3  ),
            .wave_len      (  capture_set.wave_len >> 3    ),
            .edge_valid    (  edge_valid                   ),
            .s_axis_tdata  (  s_axis_tdata[i*128+:128]     ),
            .s_axis_tvalid (  s_axis_tvalid                ),
            .status        (  status[i]                    ),
            .axis_master   (  axis_lane_if[i]              )
        );

        channal_packet #
        (
            .CHANNAL_NUM  (  i                                 )
        )
        channal_packetEx
        (
            .clk          (  clk                               ),
            .trg          (  trg_r0                            ),
            .adc_width    (  capture_set.adc12qj1600_width     ),
            .save_channal (  capture_set.save_channal[i*2+:2]  ),
            .first_pos    (  capture_set.first_pos >> 3        ),
            .first_len    (  capture_set.first_len >> 3        ),
            .second_pos   (  (edge_pos - 2)                    ),  // 单位：8
            .status       (  status[4+i]                       ),
            .axis_slave   (  axis_lane_if[i]                   ),
            .axis_first   (  axis_first_if[i]                  ),
            .axis_second  (  axis_second_if[i]                 )
        );
    end
endgenerate

/*
 * 需要响应给spi_master 读的数据
 */

reg rst = 0;
initial
begin
    rst = 0; #1us;
    rst = 1; #1us;
    rst = 0; #1us;
end

endmodule
