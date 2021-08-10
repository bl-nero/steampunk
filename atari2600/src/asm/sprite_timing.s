.include "atari2600.inc"

Reset:
            lda #0
            tax
:           dex
            sta $00,x
            bne :-
            dex
            txs
            lda #$0E
            sta COLUBK
            lda #$9C
            sta COLUPF
            lda #$E2
            sta COLUP0

            lda #%01010000
            sta PF0
            lda #%10101010
            sta PF1
            lda #%01010101
            sta PF2

StartOfFrame:
            ldy #0

            ; Start vertical blanking.
            lda #%01000010
            sta VBLANK

            ; Emit 3 scanlines of VSYMC.
            lda #2
            sta VSYNC
            sta WSYNC
            sta WSYNC
            sta WSYNC

            ; Set timer to go off after 31 scanlines (a little bit above 43*64 ticks)
            lda #43
            sta TIM64T


            ; Emit vertical blank until the timer goes off.
            lda #0
            sta VSYNC           
:           lda INTIM
            bpl :-
            sta WSYNC
            lda #0
            sta VBLANK

            ; Emit the test pattern.
            lda #%00000000
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            nop
            nop
            nop
            nop
            nop
            nop
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            nop
            nop
            nop
            nop
            nop
            bit $80
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            nop
            nop
            nop
            nop
            nop
            nop
            bit $80
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            bit $80
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            ; Right part of the screen.
            lda #%00000000
            sta GRP0
            ldx #10
:           dex
            bne :-
            nop
            nop
            nop
            nop
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0

            ; Shift by a couple of bytes to prevent next loop from being
            ; improperly cycled (crossing page boundary).
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            sta WSYNC

            lda #%00000000
            sta GRP0
            ldx #10
:           dex
            bne :-
            nop
            nop
            nop
            bit $80
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            ldx #10
:           dex
            bne :-
            nop
            nop
            nop
            nop
            nop
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            ldx #10
:           dex
            bne :-
            nop
            nop
            nop
            nop
            bit $80
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            ldx #10
:           dex
            bne :-
            nop
            nop
            nop
            nop
            nop
            nop
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            ldx #10
:           dex
            bne :-
            nop
            nop
            nop
            nop
            nop
            bit $80
            sta RESP0
            ; Note: there is a significant discrepancy between how Stella treats
            ; RESPx strobes close to the right edge and what seems correct from
            ; the analysis of the TIA diagrams and Andrew Towers' TIA notes.
            ; Stella emulator immediately draws both the "overflowing" part of
            ; the sprite immediately on the next line, even though the first
            ; copy is not supposed to appear until a full scan line's worth of
            ; pixels is generated. So I'm deliberately avoiding this case by
            ; strobing WSYNC twice in such cases; if this becomes a problem,
            ; I'll deal with it later.
            sta WSYNC
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            ldx #10
:           dex
            bne :-
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            sta RESP0
            sta WSYNC
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            lda #%00000000
            sta GRP0
            ldx #10
:           dex
            bne :-
            nop
            nop
            nop
            nop
            nop
            nop
            bit $80
            sta RESP0
            sta WSYNC
            lda #%11111111
            sta GRP0
            sta WSYNC

            ; Check GRPx delay
            lda #%00000000
            sta GRP0
            sta WSYNC

            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            nop
            sta RESP0
            sta WSYNC

            lda #%11111111
            nop
            nop
            nop
            nop
            nop
            nop
            sta GRP0
            nop
            lda #%00000000
            sta GRP0
            sta WSYNC

            lda #%11111111
            nop
            nop
            nop
            nop
            nop
            nop
            sta GRP0
            bit $80
            lda #%00000000
            sta GRP0
            sta WSYNC

            lda #%11111111
            nop
            nop
            nop
            nop
            nop
            nop
            sta GRP0
            nop
            nop
            lda #%00000000
            sta GRP0
            sta WSYNC

            lda #%11111111
            nop
            nop
            nop
            nop
            nop
            nop
            sta GRP0
            nop
            bit $80
            lda #%00000000
            sta GRP0
            sta WSYNC

            lda #%11111111
            nop
            nop
            nop
            nop
            nop
            nop
            sta GRP0
            nop
            nop
            nop
            lda #%00000000
            sta GRP0
            sta WSYNC

            ; Wait for the remaining scanlines
            ldx #(192-41)
:           sta WSYNC
            dex
            bne :-

            ; Start vertical blanking.
            lda #%01000010
            sta VBLANK

            ; Wait for 35*64 cycles, which should be just short of 30 lines of overscan.
            lda #35
            sta TIM64T
:           lda INTIM
            bpl :-
            sta WSYNC

            jmp StartOfFrame

.segment "VECTORS"
            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Reset          ; IRQ