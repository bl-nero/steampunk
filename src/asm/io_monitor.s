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
            lda #$94
            sta COLUPF

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

            ; Wait for 192 scanlines
            ldx #192
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