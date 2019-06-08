; This file is based on Andrew Davie's tutorial
; (https://www.randomterrain.com/atari-2600-memories-tutorial-andrew-davie-08.html).

.include "atari2600.inc"

Reset:
            ldx #0
StartOfFrame:
            ; Start vertical blanking.
            lda #%01000010
            sta VBLANK

            ; Emit 3 scanlines of VSYMC.
            lda #2
            sta VSYNC
            sta WSYNC
            sta WSYNC
            sta WSYNC

            ; Emit 37 scanlines of vertical blank.
            lda #0
            sta VSYNC           
            .repeat 37
                sta WSYNC
            .endrepeat

            ; Emit 192 scanlines of picture. Increment color with each line so
            ; that each line gets a different color, until we run out of them
            ; and start over. Note that we increment the color twice, since
            ; only even color numbers are actually distinguishable.
            lda #0
            sta VBLANK
            .repeat 192
                stx COLUBK
                sta WSYNC
                inx
                inx
            .endrepeat
 
            ; Start vertical blanking.
            lda #%01000010
            sta VBLANK

            ; Emit 30 scanlines of overscan.
            .repeat 30
                sta WSYNC
            .endrepeat

            jmp StartOfFrame

.segment "VECTORS"

            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Reset          ; IRQ