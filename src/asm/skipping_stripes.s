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
            ; 32 * 6 = 192, so the stripe loop should emit 192 lines of picture.
            ; ldx 32
            ldx #32

@stripeLoop:sty COLUBK
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
            bne @stripeLoop

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