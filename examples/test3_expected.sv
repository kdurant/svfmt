`timescale  1 ns/1 ps

module sim_flash_manage();

reg clk = 0;
always
    #(1s / 100_000_000 / 2) clk = ~clk;

flash_manage flash_manage_Ex01
(
    .clk           (  clk                 ),
    .rst           (  rst                 ),
    .spi_flash     (  spi_flash_if.slave  ),
    .axis_wr_in    (  axis_bit_data       ),
    .axis_flash_wr (  axis_spi_wr_data    ),
    .axis_flash_rd (  axis_spi_rd_data    ),
    .axis_rd_out   (  axis_rd_data        )
);

// =========================================================================
// DUT: spi_master_core
// =========================================================================
spi_master_core #
(
    .SYS_FREQ    (  100_000_000       ),
    .SPI_FREQ    (  10_000_000        )
)
spi_master_core_Ex01
(
    .clk         (  clk               ),
    .rst         (  rst               ),
    .axis_slave  (  axis_spi_wr_data  ),
    .axis_master (  axis_spi_rd_data  ),
    .sclk        (  sclk              ),
    .scs         (  scs               ),
    .mosi        (  sio0              ),
    .miso        (  sio1              ),
    .wp          (  sio2              ),
    .sio3        (  sio3              )
);

endmodule
