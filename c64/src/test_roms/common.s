.include "c64.inc"
.include "common.inc"

.zeropage

.export FillScreenPage
FillScreenPage: .res 2

; ==============================================================================

.code

; ------------------------------------------------------------------------------

; Initializes the VIC chip and sets the screen to blue.
.export Init
.proc Init
            lda #COL_BLUE
            sta VIC_BG_COLOR0
            lda #COL_LIGHT_BLUE
            sta VIC_BORDERCOLOR
            lda #%00011011
            sta VIC_CTRL1
            lda #%00001000
            sta VIC_CTRL2
            rts
.endproc

; ------------------------------------------------------------------------------

; Fills a 1KiB area with a byte stored in the accumulator. The start address
; should be stored at FillScreenPage. The procedure clobbers X and Y registers,
; as well as FillScreenPage.
.export FillScreen
.proc FillScreen
            ldx #4                      ; 4 pages = 1KiB
PageLoop:   ldy #0
Loop:       sta (FillScreenPage),y      ; Fill one page
            iny
            bne Loop
            inc FillScreenPage+1        ; Next page
            dex
            bne PageLoop
            rts
.endproc
