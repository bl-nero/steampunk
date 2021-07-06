.include "atari2600.inc"

Reset:
            lda #0
            tax
:           dex
            sta $00,X
            bne :-
StartOfFrame:
            ldy #0

            ; Set timer to go off after 41 scanlines (a little bit above 3*1024 ticks)
            lda #3
            sta T1024T

            ; Start vertical blanking.
            lda #%01000010
            sta VBLANK

            ; Emit 3 scanlines of VSYMC.
            lda #2
            sta VSYNC
            sta WSYNC
            sta WSYNC
            sta WSYNC

            ; Emit vertical blank until the timer goes off.
            lda #0
            sta VSYNC           
:           lda INTIM
            bpl :-
            sta WSYNC
            lda #0
            sta VBLANK

            ; Emit two sections of 16 stripes.
            ; 16 * 6 * 2 = 192, so the stripe loop should emit 192 lines of
            ; picture.
            ldx #16
@stripeLoop1:
            sty COLUBK
            iny
            iny
            lda #0
            sta WSYNC
            sta COLUBK
            lda #5
            sta TIM64T
:           lda INTIM
            bpl :-
            dex
            sta WSYNC
            bne @stripeLoop1

            ; In the second section, we do the waiting in a different way: we
            ; use WSYNC and INTIM at the same time. This is a regression test
            ; for a bug that caused RIOT clock to be only triggered when CPU is
            ; active.

            ldx #16
@stripeLoop2:
            sty COLUBK
            iny
            iny
            lda #0
            sta WSYNC
            sta COLUBK
            lda #5
            sta TIM64T
            sta WSYNC
            sta WSYNC
            sta WSYNC
:           lda INTIM
            bpl :-
            dex
            sta WSYNC
            bne @stripeLoop2

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