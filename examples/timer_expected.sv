`timescale  1 ns/1 ps

module timer_parameter #
(
    parameter SYS_FREQ    = 125_000_000,
    parameter PERIOD_TIME = 1_000,      // unit: ns
    parameter TIMEOUT_CNT = 1           // 超时信号高电平持续时间，unit: 时钟周期个数
)
(
    input          clk,

    input  [03:00] mode,

    input          enable,
    input          clear,

    output reg     timeout_flag = 0
);

localparam PERIOD_TIME_CNT = PERIOD_TIME/(1_000_000_000/SYS_FREQ);

reg [31:00] cnt   = 0;
reg [03:00] state = 0;

reg [1:0] enable_r = 2'b00;
wire      enable_rise;
wire      enable_fall;

assign enable_rise = enable_r[1:0] == 2'b01;
assign enable_fall = enable_r[1:0] == 2'b10;
always @(posedge clk)
    enable_r <= {enable_r[0], enable};

always @(posedge clk)
begin
    case(mode)
        4'b0000 :
        begin
            if(cnt < PERIOD_TIME_CNT)
                cnt <= cnt + 1'b1;
            else
                cnt <= 0;
        end
        4'b0001 :
        begin
            if(enable)
            begin
                if(cnt < PERIOD_TIME_CNT)
                    cnt <= cnt + 1'b1;
            end
            else
                cnt <= 0;
        end
        4'b0010 :
        begin
            case(state)
                4'b0000 :
                begin
                    if(~enable)
                        cnt <= 0;

                    if(enable_rise)
                        state <= 1'b1;
                    else
                        state <= 1'b0;
                end
                4'b0001 :
                begin
                    if(cnt < PERIOD_TIME_CNT)
                        cnt <= cnt + 1'b1;
                    else
                        state <= 1'b0;
                end
                default :
                begin
                end
            endcase
        end
        4'b0011 :
        begin
            if(enable)
            begin
                if(clear)
                    cnt <= 0;
                else
                begin
                    if(cnt < PERIOD_TIME_CNT)
                        cnt <= cnt + 1'b1;
                    else
                        cnt <= 0;
                end
            end
            else
                cnt <= 0;
        end
        default : cnt <= 0;
    endcase
end

always @(posedge clk)
begin
    case(mode)
        4'b0000 : timeout_flag <= (cnt > PERIOD_TIME_CNT - 2 - TIMEOUT_CNT) && (cnt <= PERIOD_TIME_CNT - 2) ? 1 : 0;
        4'b0001 : timeout_flag <= (cnt > PERIOD_TIME_CNT - 2 - TIMEOUT_CNT) && (cnt <= PERIOD_TIME_CNT - 2) && enable ? 1 : 0;
        4'b0010 : timeout_flag <= (cnt > PERIOD_TIME_CNT - 2 - TIMEOUT_CNT) && (cnt <= PERIOD_TIME_CNT - 2) ? 1 : 0;
        4'b0011 : timeout_flag <= (cnt > PERIOD_TIME_CNT - 2 - TIMEOUT_CNT) && (cnt <= PERIOD_TIME_CNT - 2) ? 1 : 0;
        default : timeout_flag <= 0;
    endcase
end

if_axi_stream #(.DATA_WIDTH(8)) user_mux_if();   // 不同来源的用户数据经过复用后的数据
if_axi_stream #(.DATA_WIDTH(8)) user_up_if();    // 按照协议封装后的数据
if_axi_stream #(.DATA_WIDTH(8)) man_decode_if(); // 经过曼彻斯特解码后的数据
if_axi_stream #(.DATA_WIDTH(8)) pmt_rx_if();     // 分离peer_status后，需要透传的数据
if_axi_stream #(.DATA_WIDTH(8)) eth_up_if();     // 最终给以太网发送的数据

endmodule
