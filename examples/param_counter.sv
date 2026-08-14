`timescale 1ns/1ps

// A parameterized synchronous counter with ANSI-style ports.
module param_counter #(
    parameter DATA_WIDTH = 8,
    parameter INIT_VALUE  = 0,
    parameter logic [3:0] MODE = 4'b0001
) (
    input  wire               clk,
    input  wire               rst_n,
    input  wire               load,
    input  wire [DATA_WIDTH-1:0] data_in,
    output logic [DATA_WIDTH-1:0] count
);

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            count <= INIT_VALUE;
        end else if (load) begin
            count <= data_in;
        end else begin
            count <= count + 1'b1;
        end
    end

endmodule
