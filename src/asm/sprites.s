.include "atari2600.inc"

.segment "ZEROPAGE"

.segment "CODE"
Reset:
            lda #0
            tax
:           dex
            sta $00,x
            bne :-
            dex
            txs

            ; Set up colors.
            lda #$0E
            sta COLUBK
            lda #$94
            sta COLUPF
            lda #$44
            sta COLUP0
            lda #$F8
            sta COLUP1

            ; Set up sprite speeds.
            lda #$F0
            sta HMP0
            lda #$10
            sta HMP1

            ; Reset the sprite horizontal position.
            ldx #5
            sta WSYNC
:           dex      ; 2
            bne :-   ; 3 (2)
            sta RESP0
            nop
            nop
            sta RESP1

StartOfFrame:
            ; Start vertical blanking.
            lda #%00000010
            sta VBLANK

            ; Emit 3 scanlines of VSYNC.
            lda #2
            sta VSYNC
            sta WSYNC
            sta WSYNC
            sta WSYNC

            ; Set timer to go off after 31 scanlines (a little bit above 43*64 ticks).
            lda #43
            sta TIM64T

            ; Emit vertical blank.
            lda #0
            sta VSYNC           


            ; Clear the sprites.
            lda #%00000000
            sta GRP0
            sta GRP1

            ; Wait until the timer goes off to start drawing the frame.
:           lda INTIM
            bpl :-
            sta WSYNC
            lda #0
            sta VBLANK

            ; Draw some dummy playfield.
            sta PF2
            lda #%01010000
            sta PF0
            lda #%10101010
            sta PF1

            ; 10 lines of margin.
            ldx #10
:           sta WSYNC
            dex
            bne :-

            ; 8 lines of sprites.
            ldx #0
:           lda Sprite0,x
            sta GRP0
            lda Sprite1,x
            sta GRP1
            inx
            cpx #8
            sta WSYNC
            bne :-

            ; Turn off sprites.
            lda #0
            sta GRP0
            sta GRP1


            ; Wait for the remaining scanlines.
            ldx #(192 - 18)
:           sta WSYNC
            dex
            bne :-

            ; Start vertical blanking.
            lda #%00000010
            sta VBLANK

            ; Move player sprites.
            sta WSYNC
            sta HMOVE

            ; Wait for 35*64 cycles, which should be just short of 30 lines of overscan.
            lda #35
            sta TIM64T
:           lda INTIM
            bpl :-
            sta WSYNC

            jmp StartOfFrame

.segment "RODATA"
Sprite0:    .byte %11111111
            .byte %10000011
            .byte %10000101
            .byte %10001001
            .byte %10010001
            .byte %10100001
            .byte %11000001
            .byte %11111111

Sprite1:    .byte %11111111
            .byte %10000011
            .byte %10000101
            .byte %10001001
            .byte %10001001
            .byte %10000101
            .byte %10000011
            .byte %11111111

.segment "VECTORS"
            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Reset          ; IRQ
