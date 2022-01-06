.macpack cbm                            ; for scrcode macro
.include "c64.inc"

SCREEN_START   = $0400
COLOR_START    = $D800
COL_WHITE      = 1
COL_BLUE       = 6
COL_LIGHT_BLUE = 14
COL_LIGHT_GREY = 15

; Offset to the center of the screen.
TEXT_OFFSET = (25 / 2) * 40 + 20 - (HELLO_LEN + 1) / 2

; ------------------------------------------------------------------------------

.zeropage

.import FillScreenPage

; ------------------------------------------------------------------------------

.code

.import FillScreen

Reset:      lda #COL_WHITE
            sta VIC_BG_COLOR0
            lda #COL_LIGHT_GREY
            sta VIC_BORDERCOLOR
            lda #%00001000
            sta VIC_CTRL2

            lda #>SCREEN_START
            sta FillScreenPage+1
            lda #<SCREEN_START
            sta FillScreenPage
            lda #32                     ; space screen code
            jsr FillScreen

            ldx #HELLO_LEN
Loop:       lda Hello-1, x
            sta SCREEN_START - 1 + TEXT_OFFSET, x
            dex
            bne Loop

            ; Set up raster IRQ interrupt for line 13, which should be the 1st
            ; VBLANK line.
            lda #13
            sta VIC_HLINE
            lda #%00011011
            sta VIC_CTRL1
            lda #%00000001
            sta VIC_IMR

            cli

End:        jmp End

; ------------------------------------------------------------------------------

.proc Irq
            lda #%00000001
            sta VIC_IRR
            inc VIC_BORDERCOLOR
            rti
.endproc

; ------------------------------------------------------------------------------

.rodata

Hello:      scrcode "hello, world!"
HELLO_LEN = * - Hello;

; ------------------------------------------------------------------------------

.segment "VECTORS"

            .word Reset          ; NMI
            .word Reset          ; RESET
            .word Irq            ; IRQ