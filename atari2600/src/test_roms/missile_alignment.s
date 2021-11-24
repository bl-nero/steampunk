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
            lda #$8E
            sta COLUPF
            lda #$52
            sta COLUP0
            lda #$D2
            sta COLUP1

            ; Set up sprite speeds.
            lda #$F0
            sta HMM0
            lda #$10
            sta HMM1

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

            ; Reset the sprite horizontal position.
            ldx #7
            sta WSYNC
:           dex      ; 2
            bne :-   ; 3 (2)
            sta RESM0
            nop
            nop
            sta RESP0
            nop
            nop
            sta RESP1
            nop
            nop
            sta RESM1

            ; Wait until the timer goes off to start drawing the frame.
:           lda INTIM
            bpl :-
            sta WSYNC
            lda #0
            sta VBLANK

            ; Draw some dummy playfield.
            lda #%01010000
            sta PF0
            lda #%10101010
            sta PF1
            lda #%01010101
            sta PF2

            ; 10 lines of margin.
            ldx #10
:           sta WSYNC
            dex
            bne :-

            ; A 20-line chain of missiles.
            lda #%00000010
            sta ENAM0
            sta ENAM1
            ldx #20
:           sta WSYNC
            sta HMOVE
            dex
            bne :-

            jsr DrawCrosshairs
            sta WSYNC

            ; Test double-sized sprites.
            lda #%00010101
            sta NUSIZ0
            sta NUSIZ1
            sta WSYNC
            sta WSYNC

            jsr DrawCrosshairs

            ; Test quad-sized sprites.
            lda #%00100111
            sta NUSIZ0
            sta NUSIZ1
            sta WSYNC
            sta WSYNC

            jsr DrawCrosshairs

            lda #%00000000
            sta NUSIZ0
            sta NUSIZ1

            ; Wait for the remaining scanlines.
            ldx #(192 - 56)
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

DrawCrosshairs:
            ; Reset missiles to players
            lda #%00000010
            sta ENAM0
            sta ENAM1
            sta RESMP0
            sta RESMP1

            ; 7 lines of the "crosshair" sprites.
            ldx #0
SpriteLoop: lda Sprite,x
            sta GRP0
            sta GRP1
            ; Show missiles on row 3...
            cpx #3
            bne :+
            lda #%00000000
            sta RESMP0
            sta RESMP1
            ; ...and hide them immediately after.
:           cpx #4
            bne :+
            lda #%00000000
            sta ENAM0
            sta ENAM1
:           inx
            cpx #7
            sta WSYNC
            bne SpriteLoop

            ; Turn off "crosshair" sprites.
            lda #0
            sta GRP0
            sta GRP1
            rts

.segment "RODATA"
Sprite:     .byte %01111111
            .byte %01001001
            .byte %01000001
            .byte %01100011
            .byte %01000001
            .byte %01001001
            .byte %01111111

.segment "VECTORS"
            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Reset          ; IRQ
