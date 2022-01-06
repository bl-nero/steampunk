.macpack cbm                            ; for scrcode macro
.include "c64.inc"

SCREEN_START   = $0400
COLOR_START    = $D800
COL_WHITE      = 1
COL_BLUE       = 6
COL_LIGHT_BLUE = 14

.zeropage

.import FillScreenPage

.code

.import FillScreen

Reset:      lda #COL_BLUE
            sta VIC_BG_COLOR0
            lda #COL_LIGHT_BLUE
            sta VIC_BORDERCOLOR
            lda #%00001000
            sta VIC_CTRL2

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