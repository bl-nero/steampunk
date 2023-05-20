.macpack cbm                            ; for scrcode macro
.include "c64.inc"
.include "common.inc"

.zeropage

.import Init
.import FillScreenPage

.code

.import FillScreen

Reset:      jsr Init

            lda #>SCREEN_START
            sta FillScreenPage+1
            lda #<SCREEN_START
            sta FillScreenPage
            lda #32                     ; space screen code
            jsr FillScreen

            ldx #(HelloEnd - Hello)
Loop:       lda Hello-1, x
            sta SCREEN_START-1, x
            lda #COL_LIGHT_BLUE
            sta COLOR_START-1, x
            dex
            bne Loop
            lda #COL_WHITE
            sta COLOR_START + HelloEnd - Hello - 1
End:        jmp End

.rodata

Hello:      scrcode "hello, world!"
HelloEnd:


.segment "VECTORS"

            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Reset          ; IRQ