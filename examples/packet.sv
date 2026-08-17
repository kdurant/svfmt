`timescale  1 ns/1 ps

module packet_sys_status 
(
    input                       clk,
    input                       rst,

    input                       enable,
    input        [31:00]        up_interval,
    input bsp::dev_info_t       dev_info,
    input bsp::dev_status_t     dev_status,
    input bsp::peer_status_t    peer_status,
    input bsp::quality_t        quality,
    input                       max3969_los,

    if_axi_stream.master        axis_master
);

if_axi_stream #(.DATA_WIDTH(8)) fifo_if ();

localparam              IDLE    = 0;
localparam              WRITE   = 1;
localparam              DLY     = 2;
localparam              OVER    = 3;
localparam              PACKET_BYTES = $bits(bsp::dev_info_t) / 8 + $bits(bsp::dev_status_t) / 8 + $bits(bsp::quality_t)/8 + $bits(bsp::peer_status_t)/8 + 1;
(* KEEP = "TRUE" *)reg     [OVER:00]       cs = 'd1, ns = 'd1;
reg     [31:00]         state_cnt = 0;
reg     [31:00]         state_cnt_n = 0;

// synthesis translate_off
reg [127:0] cs_STRING = 0;
always @(*)
begin
    case(1'b1)
        cs[IDLE]: cs_STRING = "IDLE";
        cs[WRITE]: cs_STRING = "WRITE";
        cs[DLY]: cs_STRING = "DLY";
        cs[OVER]: cs_STRING = "OVER";
        default: cs_STRING = "XXXX";
    endcase
end
// synthesis translate_on

// synthesis translate_off
assert property (@(posedge clk) disable iff (rst)
    $onehot(cs))
    else $error("packet_sys_status: cs is not one-hot");

assert property (@(posedge clk) disable iff (rst)
    fifo_if.tlast |-> fifo_if.tvalid)
    else $error("packet_sys_status: tlast asserted without tvalid");

assert property (@(posedge clk) disable iff (rst)
    (cs[WRITE] && (state_cnt == PACKET_BYTES - 1)) |=> fifo_if.tlast)
    else $error("packet_sys_status: tlast not asserted at packet end");

assert property (@(posedge clk) disable iff (rst)
    (cs[WRITE] && (state_cnt == PACKET_BYTES - 1)) |=> !cs[WRITE])
    else $error("packet_sys_status: WRITE state length does not match packet size");
// synthesis translate_on

always @(posedge clk)
begin
    cs <= ns;
end

always @(*)
begin
    ns = 'd0;
    case(1'b1)
        cs[IDLE]:
        begin
            if(enable)
                ns[WRITE] = 1'b1;
            else
                ns[IDLE] = 1'b1;
        end
        cs[WRITE]:
        begin
            if(state_cnt >= PACKET_BYTES - 1)
                ns[DLY] = 1'b1;
            else
                ns[WRITE] = 1'b1;
        end
        cs[DLY]: 
        begin
            `ifdef MODELSIM
            if(state_cnt >= 2000)
                ns[OVER] = 1'b1;
            else
                ns[DLY] = 1'b1;
            `else
            // if(state_cnt >= 125_000_000) // 1s
            if(state_cnt >= up_interval)

                ns[OVER] = 1'b1;
            else
                ns[DLY] = 1'b1;
            `endif
        end
        default:
            ns[IDLE] = 1'b1;
    endcase
end

always @ (posedge clk)
begin
    state_cnt <= state_cnt_n;
end

always @ (*)
begin
    if (cs != ns)
        state_cnt_n = 0;
    else
        state_cnt_n = state_cnt + 1'b1;
end

always @ (posedge clk)
begin
    fifo_if.tvalid <= cs[WRITE];
    fifo_if.tlast <= cs[WRITE] & !ns[WRITE];
end

always @ (posedge clk)
begin
    if(cs[WRITE])
    begin
        case (state_cnt)
            'd00: fifo_if.tdata  <= dev_info.fpga_version[255 -: 8];
            'd01: fifo_if.tdata  <= dev_info.fpga_version[247 -: 8];
            'd02: fifo_if.tdata  <= dev_info.fpga_version[239 -: 8];
            'd03: fifo_if.tdata  <= dev_info.fpga_version[231 -: 8];
            'd04: fifo_if.tdata  <= dev_info.fpga_version[223 -: 8];
            'd05: fifo_if.tdata  <= dev_info.fpga_version[215 -: 8];
            'd06: fifo_if.tdata  <= dev_info.fpga_version[207 -: 8];
            'd07: fifo_if.tdata  <= dev_info.fpga_version[199 -: 8];
            'd08: fifo_if.tdata  <= dev_info.fpga_version[191 -: 8];
            'd09: fifo_if.tdata  <= dev_info.fpga_version[183 -: 8];
            'd10: fifo_if.tdata  <= dev_info.fpga_version[175 -: 8];
            'd11: fifo_if.tdata  <= dev_info.fpga_version[167 -: 8];
            'd12: fifo_if.tdata  <= dev_info.fpga_version[159 -: 8];
            'd13: fifo_if.tdata  <= dev_info.fpga_version[151 -: 8];
            'd14: fifo_if.tdata  <= dev_info.fpga_version[143 -: 8];
            'd15: fifo_if.tdata  <= dev_info.fpga_version[135 -: 8];
            'd16: fifo_if.tdata  <= dev_info.fpga_version[127 -: 8];
            'd17: fifo_if.tdata  <= dev_info.fpga_version[119 -: 8];
            'd18: fifo_if.tdata  <= dev_info.fpga_version[111 -: 8];
            'd19: fifo_if.tdata  <= dev_info.fpga_version[103 -: 8];
            'd20: fifo_if.tdata  <= dev_info.fpga_version[95 -: 8];
            'd21: fifo_if.tdata  <= dev_info.fpga_version[87 -: 8];
            'd22: fifo_if.tdata  <= dev_info.fpga_version[79 -: 8];
            'd23: fifo_if.tdata  <= dev_info.fpga_version[71 -: 8];
            'd24: fifo_if.tdata  <= dev_info.fpga_version[63 -: 8];
            'd25: fifo_if.tdata  <= dev_info.fpga_version[55 -: 8];
            'd26: fifo_if.tdata  <= dev_info.fpga_version[47 -: 8];
            'd27: fifo_if.tdata  <= dev_info.fpga_version[39 -: 8];
            'd28: fifo_if.tdata  <= dev_info.fpga_version[31 -: 8];
            'd29: fifo_if.tdata  <= dev_info.fpga_version[23 -: 8];
            'd30: fifo_if.tdata  <= dev_info.fpga_version[15 -: 8];
            'd31: fifo_if.tdata  <= dev_info.fpga_version[7 -: 8];

            'd32: fifo_if.tdata  <= dev_info.fpga_dna[127 -: 8];
            'd33: fifo_if.tdata  <= dev_info.fpga_dna[119 -: 8];
            'd34: fifo_if.tdata  <= dev_info.fpga_dna[111 -: 8];
            'd35: fifo_if.tdata  <= dev_info.fpga_dna[103 -: 8];
            'd36: fifo_if.tdata  <= dev_info.fpga_dna[95 -: 8];
            'd37: fifo_if.tdata  <= dev_info.fpga_dna[87 -: 8];
            'd38: fifo_if.tdata  <= dev_info.fpga_dna[79 -: 8];
            'd39: fifo_if.tdata  <= dev_info.fpga_dna[71 -: 8];
            'd40: fifo_if.tdata  <= dev_info.fpga_dna[63 -: 8];
            'd41: fifo_if.tdata  <= dev_info.fpga_dna[55 -: 8];
            'd42: fifo_if.tdata  <= dev_info.fpga_dna[47 -: 8];
            'd43: fifo_if.tdata  <= dev_info.fpga_dna[39 -: 8];
            'd44: fifo_if.tdata  <= dev_info.fpga_dna[31 -: 8];
            'd45: fifo_if.tdata  <= dev_info.fpga_dna[23 -: 8];
            'd46: fifo_if.tdata  <= dev_info.fpga_dna[15 -: 8];
            'd47: fifo_if.tdata  <= dev_info.fpga_dna[7 -: 8];

            'd48: fifo_if.tdata  <= dev_info.device_sn[127 -: 8];
            'd49: fifo_if.tdata  <= dev_info.device_sn[119 -: 8];
            'd50: fifo_if.tdata  <= dev_info.device_sn[111 -: 8];
            'd51: fifo_if.tdata  <= dev_info.device_sn[103 -: 8];
            'd52: fifo_if.tdata  <= dev_info.device_sn[95 -: 8];
            'd53: fifo_if.tdata  <= dev_info.device_sn[87 -: 8];
            'd54: fifo_if.tdata  <= dev_info.device_sn[79 -: 8];
            'd55: fifo_if.tdata  <= dev_info.device_sn[71 -: 8];
            'd56: fifo_if.tdata  <= dev_info.device_sn[63 -: 8];
            'd57: fifo_if.tdata  <= dev_info.device_sn[55 -: 8];
            'd58: fifo_if.tdata  <= dev_info.device_sn[47 -: 8];
            'd59: fifo_if.tdata  <= dev_info.device_sn[39 -: 8];
            'd60: fifo_if.tdata  <= dev_info.device_sn[31 -: 8];
            'd61: fifo_if.tdata  <= dev_info.device_sn[23 -: 8];
            'd62: fifo_if.tdata  <= dev_info.device_sn[15 -: 8];
            'd63: fifo_if.tdata  <= dev_info.device_sn[07 -: 8];

            'd64: fifo_if.tdata  <= dev_info.device_type[31 -: 8];
            'd65: fifo_if.tdata  <= dev_info.device_type[23 -: 8];
            'd66: fifo_if.tdata  <= dev_info.device_type[15 -: 8];
            'd67: fifo_if.tdata  <= dev_info.device_type[07 -: 8];

            'd68: fifo_if.tdata  <= dev_status.up_time[31 -: 8];
            'd69: fifo_if.tdata  <= dev_status.up_time[23 -: 8];
            'd70: fifo_if.tdata  <= dev_status.up_time[15 -: 8];
            'd71: fifo_if.tdata  <= dev_status.up_time[07 -: 8];

            'd72: fifo_if.tdata  <= dev_status.xadc[0][15 -: 8];
            'd73: fifo_if.tdata  <= dev_status.xadc[0][07 -: 8];
            'd74: fifo_if.tdata  <= dev_status.xadc[1][15 -: 8];
            'd75: fifo_if.tdata  <= dev_status.xadc[1][07 -: 8];
            'd76: fifo_if.tdata  <= dev_status.xadc[2][15 -: 8];
            'd77: fifo_if.tdata  <= dev_status.xadc[2][07 -: 8];
            'd78: fifo_if.tdata  <= dev_status.xadc[3][15 -: 8];
            'd79: fifo_if.tdata  <= dev_status.xadc[3][07 -: 8];
            'd80: fifo_if.tdata  <= dev_status.xadc[4][15 -: 8];
            'd81: fifo_if.tdata  <= dev_status.xadc[4][07 -: 8];
            'd82: fifo_if.tdata  <= dev_status.xadc[5][15 -: 8];
            'd83: fifo_if.tdata  <= dev_status.xadc[5][07 -: 8];

            'd84: fifo_if.tdata  <= peer_status.hrssi[15 -: 8];
            'd85: fifo_if.tdata  <= peer_status.hrssi[07 -: 8];
            'd86: fifo_if.tdata  <= dev_status.srssi[15 -: 8];
            'd87: fifo_if.tdata  <= dev_status.srssi[07 -: 8];

            'd88: fifo_if.tdata  <= quality.period3[31 -: 8];
            'd89: fifo_if.tdata  <= quality.period3[23 -: 8];
            'd90: fifo_if.tdata  <= quality.period3[15 -: 8];
            'd91: fifo_if.tdata  <= quality.period3[07 -: 8];
            'd92: fifo_if.tdata  <= quality.period4[31 -: 8];
            'd93: fifo_if.tdata  <= quality.period4[23 -: 8];
            'd94: fifo_if.tdata  <= quality.period4[15 -: 8];
            'd95: fifo_if.tdata  <= quality.period4[07 -: 8];
            'd96: fifo_if.tdata  <= quality.period5[31 -: 8];
            'd97: fifo_if.tdata  <= quality.period5[23 -: 8];
            'd98: fifo_if.tdata  <= quality.period5[15 -: 8];
            'd99: fifo_if.tdata  <= quality.period5[07 -: 8];
            'd100: fifo_if.tdata <= quality.period6[31 -: 8];
            'd101: fifo_if.tdata <= quality.period6[23 -: 8];
            'd102: fifo_if.tdata <= quality.period6[15 -: 8];
            'd103: fifo_if.tdata <= quality.period6[07 -: 8];
            'd104: fifo_if.tdata <= quality.period7[31 -: 8];
            'd105: fifo_if.tdata <= quality.period7[23 -: 8];
            'd106: fifo_if.tdata <= quality.period7[15 -: 8];
            'd107: fifo_if.tdata <= quality.period7[07 -: 8];
            'd108: fifo_if.tdata <= quality.period8[31 -: 8];
            'd109: fifo_if.tdata <= quality.period8[23 -: 8];
            'd110: fifo_if.tdata <= quality.period8[15 -: 8];
            'd111: fifo_if.tdata <= quality.period8[07 -: 8];
            'd112: fifo_if.tdata <= quality.period9[31 -: 8];
            'd113: fifo_if.tdata <= quality.period9[23 -: 8];
            'd114: fifo_if.tdata <= quality.period9[15 -: 8];
            'd115: fifo_if.tdata <= quality.period9[07 -: 8];
            'd116: fifo_if.tdata <= quality.period10[31 -: 8];
            'd117: fifo_if.tdata <= quality.period10[23 -: 8];
            'd118: fifo_if.tdata <= quality.period10[15 -: 8];
            'd119: fifo_if.tdata <= quality.period10[07 -: 8];
            'd120: fifo_if.tdata <= quality.period11[31 -: 8];
            'd121: fifo_if.tdata <= quality.period11[23 -: 8];
            'd122: fifo_if.tdata <= quality.period11[15 -: 8];
            'd123: fifo_if.tdata <= quality.period11[07 -: 8];
            'd124: fifo_if.tdata <= quality.period12[31 -: 8];
            'd125: fifo_if.tdata <= quality.period12[23 -: 8];
            'd126: fifo_if.tdata <= quality.period12[15 -: 8];
            'd127: fifo_if.tdata <= quality.period12[07 -: 8];
            'd128: fifo_if.tdata <= quality.period13[31 -: 8];
            'd129: fifo_if.tdata <= quality.period13[23 -: 8];
            'd130: fifo_if.tdata <= quality.period13[15 -: 8];
            'd131: fifo_if.tdata <= quality.period13[07 -: 8];
            'd132: fifo_if.tdata <= quality.period_other[31 -: 8];
            'd133: fifo_if.tdata <= quality.period_other[23 -: 8];
            'd134: fifo_if.tdata <= quality.period_other[15 -: 8];
            'd135: fifo_if.tdata <= quality.period_other[07 -: 8];

            'd136: fifo_if.tdata  <= peer_status.up_time[31 -: 8];
            'd137: fifo_if.tdata  <= peer_status.up_time[23 -: 8];
            'd138: fifo_if.tdata  <= peer_status.up_time[15 -: 8];
            'd139: fifo_if.tdata  <= peer_status.up_time[07 -: 8];
            'd140: fifo_if.tdata  <= max3969_los;
            'd141: fifo_if.tdata  <= dev_status.pmt_gain[15 -: 8];
            'd142: fifo_if.tdata  <= dev_status.pmt_gain[07 -: 8];
            'd143: fifo_if.tdata  <= dev_status.ld_voltage[15 -: 8];
            'd144: fifo_if.tdata  <= dev_status.ld_voltage[07 -: 8];

            default: fifo_if.tdata <= 0;
        endcase
    end
end

localparam              FIFO_DEPTH = 1024;
localparam              FIFO_DATA_WIDTH = 8;
localparam              FIFO_COUNT_WIDTH = $clog2(FIFO_DEPTH) + 1;
xpm_fifo_axis #
(
    .CLOCKING_MODE          (  "common_clock"                             ), // common_clock, independent_clock
    .FIFO_DEPTH             (  FIFO_DEPTH                                 ), // 16 - 4194304
    .FIFO_MEMORY_TYPE       (  "auto"                                     ), // auto, block, distributed, ultra
    .PACKET_FIFO            (  "true"                                     ), // true, false
    .RD_DATA_COUNT_WIDTH    (  FIFO_COUNT_WIDTH                           ),
    .TDATA_WIDTH            (  FIFO_DATA_WIDTH                            ),
    .TDEST_WIDTH            (  1                                          ), // 1 - 32
    .USE_ADV_FEATURES       (  "1404"                                     ),
    .WR_DATA_COUNT_WIDTH    (  FIFO_COUNT_WIDTH                           )
)
xpm_fifo_axis_Ex01 
(
    .s_aclk                 (  clk                                        ),
    .s_aresetn              (  ~rst                                       ),

    .s_axis_tvalid          (  fifo_if.tvalid                             ),
    .s_axis_tready          (  fifo_if.tready                             ),
    .s_axis_tdata           (  fifo_if.tdata                              ),
    .s_axis_tlast           (  fifo_if.tlast                              ),

    .m_aclk                 (  clk                                        ),
    .m_axis_tvalid          (  axis_master.tvalid                         ),
    .m_axis_tready          (  axis_master.tready                         ),
    .m_axis_tdata           (  axis_master.tdata                          ),
    .m_axis_tlast           (  axis_master.tlast                          ),
    .rd_data_count_axis     (  axis_master.tlen[FIFO_COUNT_WIDTH-1:00]    )
);

assign                  axis_master.tlen[FIFO_COUNT_WIDTH +: 16-FIFO_COUNT_WIDTH] = 0;
assign                  axis_master.tdest = 4'd1;
endmodule
