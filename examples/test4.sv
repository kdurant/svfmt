module tb_sata_8b10b;

  import sata_pkg::*;

  int errors = 0, checks = 0; 

  sata_8b10b u_enc (); // instance to call hierarchical functions

  task automatic check_code(input string name, input logic [9:0] exp, input logic [9:0] got);
    checks++;
    if (exp !== got) begin errors++;
      $display("FAIL %-28s exp=%03X got=%03X", name, exp, got);
    end else $display("PASS %-28s = %03X", name, got);
  endtask

  task automatic check_rdeq(input string name, input logic exp, input logic got);
    checks++;
    if (exp !== got) begin errors++;
      $display("FAIL %-28s exp=%0d got=%0d", name, exp, got);
    end
  endtask

  initial begin
    logic [9:0] code;
    logic       rd;
    logic [8:0] dec;
    logic [15:0] fail_rts = 0;

    // --- encode known vectors ---
    code = u_enc.encode_8b10b(8'hBC, 1'b1, 1'b0); // K28.5 RD-
    check_code("K28.5 RD-", 10'h17C, code);
    rd   = u_enc.encode_rd_after(code, 1'b0);
    check_rdeq("K28.5 RD- next", 1'b1, rd);
    code = u_enc.encode_8b10b(8'hBC, 1'b1, 1'b1); // K28.5 RD+
    check_code("K28.5 RD+", 10'h283, code);
    rd   = u_enc.encode_rd_after(code, 1'b1);
    check_rdeq("K28.5 RD+ next", 1'b0, rd);

    code = u_enc.encode_8b10b(8'h7C, 1'b1, 1'b0);
    check_code("K28.3 RD-", 10'h33C, code);
    code = u_enc.encode_8b10b(8'h7C, 1'b1, 1'b1);
    check_code("K28.3 RD+", 10'h0C3, code);

    code = u_enc.encode_8b10b(8'hB5, 1'b0, 1'b0);
    check_code("D21.5 RD-", 10'h155, code);
    code = u_enc.encode_8b10b(8'h4A, 1'b0, 1'b0);
    check_code("D10.2 RD-", 10'h2AA, code);
    code = u_enc.encode_8b10b(8'h9C, 1'b0, 1'b0);
    check_code("D28.3 RD-", 10'h2DC, code);
    code = u_enc.encode_8b10b(8'h7B, 1'b0, 1'b0);
    check_code("D27.3 RD-", 10'h31B, code);
    code = u_enc.encode_8b10b(8'h37, 1'b0, 1'b0);
    check_code("D5.3  RD-", 10'h257, code);
    code = u_enc.encode_8b10b(8'h55, 1'b0, 1'b0);
    check_code("D5.5  RD-", 10'h295, code);

    // --- full round trip: encode every byte (both start RD) then decode ---
    fail_rts = 0;
    for (int rd0 = 0; rd0 < 2; ++rd0) begin
      for (int i = 0; i < 256; ++i) begin
        code = u_enc.encode_8b10b(i[7:0], 1'b0, rd0[0]);
        dec  = u_enc.decode_8b10b(code);
        if (dec[7:0] != i[7:0] || dec[8] != 1'b0) begin
          if (fail_rts < 8)
            $display("FAIL roundtrip data byte=%02X rdstart=%0d code=%03X dec=%02X k=%0d",
                     i, rd0, code, dec[7:0], dec[8]);
          fail_rts++;
        end else checks++;
      end
    end
    if (fail_rts != 0) begin
      errors += fail_rts;
      $display("FAIL roundtrip: %0d data bytes failed", fail_rts);
    end else $display("PASS roundtrip all 512 data enc/dec");

    // --- decode K ---
    code = u_enc.encode_8b10b(8'hBC, 1'b1, 1'b0);
    dec  = u_enc.decode_8b10b(code);
    checks++;
    if (dec[8] != 1'b1 || dec[7:0] != 8'hBC) begin errors++;
      $display("FAIL K28.5 decode got K=%0d data=%02X", dec[8], dec[7:0]);
    end
    code = u_enc.encode_8b10b(8'h7C, 1'b1, 1'b1);
    dec  = u_enc.decode_8b10b(code);
    checks++;
    if (dec[8] != 1'b1 || dec[7:0] != 8'h7C) begin errors++;
      $display("FAIL K28.3 decode got K=%0d data=%02X", dec[8], dec[7:0]);
    end

    // --- invalid code detection ---
    dec = u_enc.decode_8b10b(10'b0000000000);
    checks++;
    if (dec[8] != 1'b1) begin errors++;
      $display("FAIL invalid code not marked");
    end

    if (errors == 0) $display("\n%s: ALL %0d CHECKS PASSED", "tb_sata_8b10b", checks);
    else            $display("\n%s: %0d/%0d CHECKS FAILED", "tb_sata_8b10b", errors, checks);
    $finish;
  end

endmodule
