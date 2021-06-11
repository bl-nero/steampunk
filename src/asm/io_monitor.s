.include "atari2600.inc"

.segment "ZEROPAGE"
SwitchesPF1: .res 1
SwitchesPF2: .res 1
Joy1PF1: .res 1
Joy1PF2: .res 1
Joy2PF1: .res 1
Joy2PF2: .res 1
Joy3PF1: .res 1
Joy3PF2: .res 1
RegTemp: .res 1

.segment "CODE"
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
            lda #%00000010
            sta VBLANK

            ; Emit 3 scanlines of VSYNC.
            lda #2
            sta VSYNC
            sta WSYNC
            sta WSYNC
            sta WSYNC

            ; Set timer to go off after 31 scanlines (a little bit above 43*64 ticks)
            lda #43
            sta TIM64T

            ; Emit vertical blank.
            lda #0
            sta VSYNC           
            
            ; Analyze the SWCHB register.
            lda SWCHB
            sta RegTemp
            lda #%01010101
            asl RegTemp
            bcc Swchb6
            ora #%10000000
Swchb6:     asl RegTemp
            bcc Swchb5
            ora #%00100000
Swchb5:     asl RegTemp
            bcc Swchb4
            ora #%00001000
Swchb4:     asl RegTemp
            bcc Swchb3
            ora #%00000010
Swchb3:     sta SwitchesPF1

            lda #%10101010
            asl RegTemp
            bcc Swchb2
            ora #%00000001
Swchb2:     asl RegTemp
            bcc Swchb1
            ora #%00000100
Swchb1:     asl RegTemp
            bcc Swchb0
            ora #%00010000
Swchb0:     asl RegTemp
            bcc SwchbDone
            ora #%01000000
SwchbDone:  sta SwitchesPF2

            ; 00111000 00111000
            ; 00101000 00101000
            ; 11111110 11111110
            ; 10101010 10101010
            ; 11111110 11111110
            ; 00101000 00101000
            ; 00111000 00111000

            ; Analyze joysticks.
            lda SWCHA
            sta RegTemp

            asl RegTemp  ; P0 right
            lda #%10101010
            bcs :+
            ora #%00000100
:           asl RegTemp  ; P0 left
            bcs :+
            ora #%01000000
:           bit INPT4
            bmi :+
            ora #%00010000
:           sta Joy2PF1
            asl RegTemp  ; P0 down
            lda #%00101000
            bcs :+
            ora #%00010000
:           sta Joy3PF1
            asl RegTemp  ; P0 up
            lda #%00101000
            bcs :+
            ora #%00010000
:           sta Joy1PF1

            asl RegTemp  ; P1 right
            lda #%01010101
            bcs :+
            ora #%00100000
:           asl RegTemp  ; P1 left
            bcs :+
            ora #%00000010
:           bit INPT5
            bmi :+
            ora #%00001000
:           sta Joy2PF2
            asl RegTemp  ; P1 down
            lda #%00010100
            bcs :+
            ora #%00001000
:           sta Joy3PF2
            asl RegTemp  ; P1 up
            lda #%00010100
            bcs :+
            ora #%00001000
:           sta Joy1PF2

            ; Wait until the timer goes off to start drawing the frame.
:           lda INTIM
            bpl :-
            sta WSYNC
            lda #0
            sta PF0
            sta PF1
            sta PF2
            sta VBLANK

            ; First, some margin.
            ldx #9
:           sta WSYNC
            dex
            bne :-

            ; Draw SWCHB top frame
            sta WSYNC
            lda #%10000000 ; 2
            sta PF0        ; 5
            lda #%11111111 ; 7
            sta PF1        ; 10
            sta PF2        ; 13
            .repeat 13
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            sta WSYNC
            lda #%10000000 ; 2
            sta PF0        ; 5
            lda #%01010101 ; 7
            sta PF1        ; 10
            lda #%10101010 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            ldx #6
SwitchesLoop:
            sta WSYNC
            lda #%10000000 ; 2
            sta PF0        ; 5
            lda SwitchesPF1; 8
            sta PF1        ; 11
            lda SwitchesPF2; 14
            sta PF2        ; 17
            .repeat 11
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50
            dex
            bne SwitchesLoop

            ; Draw SWCHB bottom frame
            sta WSYNC
            lda #%10000000 ; 2
            sta PF0        ; 5
            lda #%01010101 ; 7
            sta PF1        ; 10
            lda #%10101010 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            sta WSYNC
            lda #%10000000 ; 2
            sta PF0        ; 5
            lda #%11111111 ; 7
            sta PF1        ; 10
            sta PF2        ; 13
            .repeat 13
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            ; Margin between switches and joysticks.
            ldx #10
:           sta WSYNC
            dex
            bne :-

            ; Draw joysticks' top frame.
            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%00111000 ; 7
            sta PF1        ; 10
            lda #%00011100 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%00101000 ; 7
            sta PF1        ; 10
            lda #%00010100 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            ; Draw joysticks' up buttons.
            ldx #6
JoysticksLoop1:
            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda Joy1PF1    ; 8
            sta PF1        ; 11
            lda Joy1PF2    ; 14
            sta PF2        ; 17
            .repeat 11
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50
            dex
            bne JoysticksLoop1

            ; Draw joysticks' middle frame #1.
            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%00101000 ; 7
            sta PF1        ; 10
            lda #%00010100 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%11111110 ; 7
            sta PF1        ; 10
            lda #%01111111 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%10101010 ; 7
            sta PF1        ; 10
            lda #%01010101 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            ; Draw joysticks' middle buttons.
            ldx #6
JoysticksLoop2:
            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda Joy2PF1    ; 8
            sta PF1        ; 11
            lda Joy2PF2    ; 14
            sta PF2        ; 17
            .repeat 11
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50
            dex
            bne JoysticksLoop2

            ; Draw joysticks' middle frame #2.
            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%10101010 ; 7
            sta PF1        ; 10
            lda #%01010101 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%11111110 ; 7
            sta PF1        ; 10
            lda #%01111111 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%00101000 ; 7
            sta PF1        ; 10
            lda #%00010100 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            ; Draw joysticks' down buttons.
            ldx #6
JoysticksLoop3:
            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda Joy3PF1    ; 8
            sta PF1        ; 11
            lda Joy3PF2    ; 14
            sta PF2        ; 17
            .repeat 11
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50
            dex
            bne JoysticksLoop3

            ; Draw joysticks' bottom frame.
            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%00101000 ; 7
            sta PF1        ; 10
            lda #%00010100 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            sta WSYNC
            lda #0         ; 2
            sta PF0        ; 5
            lda #%00111000 ; 7
            sta PF1        ; 10
            lda #%00011100 ; 12
            sta PF2        ; 15
            .repeat 12
            nop
            .endrepeat     ; 39
            lda #0         ; 41
            sta PF0        ; 44
            sta PF1        ; 47
            sta PF2        ; 50

            ; Wait for the remaining scanlines
            ldx #(192-58)
:           sta WSYNC
            dex
            bne :-

            ; Start vertical blanking.
            lda #%00000010
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