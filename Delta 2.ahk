#Requires AutoHotkey v2.0
;                                   ██████╗ ███████╗██╗  ████████╗ █████╗     ██████╗ 
;                                   ██╔══██╗██╔════╝██║  ╚══██╔══╝██╔══██╗    ╚════██╗
;                                   ██║  ██║█████╗  ██║     ██║   ███████║     █████╔╝
;                                   ██║  ██║██╔══╝  ██║     ██║   ██╔══██║    ██╔═══╝ 
;                                   ██████╔╝███████╗███████╗██║   ██║  ██║    ███████╗
;                                   ╚═════╝ ╚══════╝╚══════╝╚═╝   ╚═╝  ╚═╝    ╚══════╝

; ╭───────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
; │                                   Windows AutoHotkey macro and keybind script.                                    │
; │                                             - Seglectic Systems 2023                                              │
; |                                                                                                                   |
; |                  Doc reference: https://www.autohotkey.com/docs/v2/Hotkeys.htm                                    |
; ╰───────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯


; ╭───────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
; │                                               App Specific Hotkeys                                                │
; ╰───────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯

; ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
; │                                                 Final Fantasy XIV                                                  │
; ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
FFXIV := "FINAL FANTASY XIV" ; FFXIV window name variable for checking if it's active



; Generic chat send function
SendChat(message) {
  if WinActive(FFXIV) {
    Send "{Enter}"  ; Send enter to open chat
    Sleep 20        ; Wait for chat to open
    Send message    ; Send message
    Sleep 50        ; Wait for message to send
    Send "{Enter}"  ; Send enter to send message
    return
  }	
}

; ╭──────────────────────────────────╮
; │  Ride Pillion Keybinds           │
; │  Targets the nearest player      │
; │  and sends /ridepillion <t> 1-3  │
; ╰──────────────────────────────────╯
; //TODO - Use the above SendChat function to send commands instead
Numpad1::{                    ; /ridepillion1
  if WinActive(FFXIV) {
    Send "{NumpadSub}"        ; Send numpadminus to activate nearest PC keybind
    Sleep 20
    Send "{Enter}"            ; Send enter to open chat
    Sleep 20
    Send "{/}"                ; Send /
    Sleep 20
    Send "ridepillion <t> 1"  ; Send ridepillion1
    Sleep 50
    Send "{Enter}"            ; Send enter to send message
  }
  return
}

; Numpad 2 = Target Nearest PC, then /ridepillion2
Numpad2::{
  if WinActive(FFXIV) {
    Send "{NumpadSub}"  ; Send numpadminus to activate nearest PC keybind
    Sleep 20
    Send "{Enter}"  ; Send enter to open chat
    Sleep 20
    Send "{/}" ; Send /
    Sleep 20
    Send "ridepillion <t> 2" ; Send 
    Sleep 50
    Send "{Enter}" ; Send enter to send message
    return
  }	
}

; Numpad 3 = Target Nearest PC, then /ridepillion3
Numpad3::{
  if WinActive(FFXIV) {
    Send "{NumpadSub}"  ; Send numpadminus to activate nearest PC keybind
    Sleep 20
    Send "{Enter}"  ; Send enter to open chat
    Sleep 20
    Send "{/}" ; Send /
    Sleep 20
    Send "ridepillion <t> 3" ; Send ridepillion1
    Sleep 50
    Send "{Enter}" ; Send enter to send message
    return
  }	
}

; Numpad 4 = Target Nearest PC, then /ridepillion4
Numpad4::{
  if WinActive(FFXIV) {
    Send "{NumpadSub}"  ; Send numpadminus to activate nearest PC keybind
    Sleep 20
    Send "{Enter}"  ; Send enter to open chat
    Sleep 20
    Send "{/}" ; Send /
    Sleep 20
    Send "ridepillion <t> 4" ; Send ridepillion1
    Sleep 50
    Send "{Enter}" ; Send enter to send message
    return
  }	
}

;Numpad Period = To reload the GentleFix PS5 controller vibration plugin
NumpadDot::{
  if WinActive(FFXIV) {
    Send "{Enter}"  ; Send enter to open chat
    Sleep 20
    Send "/gentle r" ; Send gentle r
    Sleep 50
    Send "{Enter}" ; Send enter to send message
    return
  }	
}


;  ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
;  │                                                        Discord                                                     │
;  ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
; //TODO - Alt+; and Win+; to open the Emoji and GIF picker respectively
; //TODO - Shift+Knob Left/Right to go up/down a Server
; //FIXME - Upon first boot, Discord is not properly shown when activation hotkey pressed (Window is black for some reason)

Discord := "Discord"                                                                                        ; Discord window name
DiscordPath := "C:\Users\" . A_UserName . "\AppData\Local\Discord\Update.exe --processStart Discord.exe"    ; Discord path (should be universal on windows)

; Activate the discord window when pressing F19 or pressing Delta + R
F20 & r::
F19::
{
  if WinActive(Discord) {
    WinHide Discord
    return
  } else{
    try{
      WinActivate Discord ; Attempt to activate window (if it's shown but inactive)
    } catch{
      try{
        WinShow Discord ; Otherwise attempt to show window (if it's hidden)
        WinActivate Discord ; And activate it
      } catch{
        ; If all else fails, run DiscordPath and activate it
        RunWait DiscordPath
        WinWaitActive Discord
      }
    }
    return
  }
}

; Rotary Encoder Left for Discord Previous Channel / DM
F16::{
  if WinActive(Discord) {
    Send "!{Up}"
    return
  }	
}

; Rotary Encoder Right for Discord Next Channel / DM
F17::{
  if WinActive(Discord) {
    Send "!{Down}"
    return
  }	
}


; ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
; │                                                    Delta + WASD                                                    │
; │                                                  Window Movement                                                   │
; ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯

; Fullscreen current window with Delta + W
F20 & w::{
  WinGetPos &X, &Y, &W, &H, "A"
  WinMove 0,0,A_ScreenWidth,A_ScreenHeight,"A"
  return
}

; Center current window at specified resolution
F20 & s::{
  winHeight := 768
  winWidth := 1024
  WinMove A_ScreenWidth/2-(winWidth/2),A_ScreenHeight/2-(winHeight/2),winWidth,winHeight,"A"
  return
}

; Move current window to left 2/3 of screen
F20 & a::{
  WinMove 0,0,A_ScreenWidth*2/3,A_ScreenHeight,"A"
  return
}

; Move current window to right 2/3 of screen
F20 & d::{
  WinMove A_ScreenWidth/3,0,A_ScreenWidth*2/3,A_ScreenHeight,"A"
  return
}	

; Move current window to left 1/3 of screen
F20 & q::{
  WinMove 0,0,A_ScreenWidth/3,A_ScreenHeight,"A"
  return
}
; Move current window to right 1/3 of screen
F20 & e::{
  WinMove A_ScreenWidth*2/3,0,A_ScreenWidth/3,A_ScreenHeight,"A"
  return
}


; ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
; │                                              Windows Explorer Hotkeys                                              │
; ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
; //FIXME - Explorer windows currently not coming into foreground focus when workflows are run

; Open Downloads folder with Delta + F
F20 & f::{
  run "C:\Users\" . A_UserName . "\Downloads"
  WinWaitActive "Downloads"
  WinActivate "Downloads"
}

; Open SegSafe shared folder with Delta + G
F20 & g::{
  run "S:\"
  WinWaitActive "S:\"
  WinActivate "S:\"
}

; Open current Windows Explorer folder in VSCode with Alt+Shift+C
; //FIXME - Unreliable, frequently doesn't properly copy path
!+c::{
  if WinActive("ahk_exe explorer.exe") {
    ; Disable user input for a moment
    Send "{Alt Up}"
    Send "{Shift Up}"
    Send "{c up}" 
    Send "{F4}"
    Send "{Ctrl Down}c{Ctrl Up}"
    Run "C:\Program Files\Microsoft VS Code\Code.exe " . A_Clipboard
  }
}

; ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
; │                                               Windows Media Controls                                               │
; ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
; Ctrl + Volume Down to Rotary Encoder Left
^F16::{
  Send "{Volume_Down}"
  return
}

; Ctrl + Volume Up to Rotary Encoder Right
^F17::{
  Send "{Volume_Up}"
  return
}

; ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
; │                                            General App Opening Hotkeys                                             │
; ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
; Show / hide Obsidian with Delta + C
ObsidianName := "Obsidian"
ObsidianExe := "ahk_exe Obsidian.exe"
ObsidianPath := 	"C:\Users\" . A_UserName . "\AppData\Local\Obsidian\Obsidian.exe"
F20 & z::{
  if WinActive(ObsidianExe,ObsidianName) {
    WinHide ObsidianName
    return
  } else{
    try{
      WinActivate ObsidianName ; Attempt to activate window (if it's shown but inactive)
    } catch{
      try{
        WinShow ObsidianName ; Otherwise attempt to show window (if it's hidden)
        WinActivate ObsidianName ; And activate it
      } catch{
        ; If all else fails, run ObsidianPath and activate it
        RunWait ObsidianPath
        WinWaitActive ObsidianName
      }
    }
    return
  }
}






; ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
; │                                               AltDrag Key Processing                                               │
; ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
; Shift + Delta > F24 + Left Click (Altdrag move window)
; In AltDrag.ini set Hotkeys=87 for F24
+F20::{
  SendLevel 2       ; Needed for F24 to work https://www.autohotkey.com/docs/v2/lib/SendLevel.htm 
  Send "{F24 Down}" ; Hold F24
  Click "Down"      ; Left click down
  KeyWait "F20"     ; Wait for F20 to be released
  Send "{F24 Up}"   ; Release F2
  Click "Up"        ; And left click
}


; ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
; │                                                   Script Reload                                                    │
; ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
F20 & BackSpace::{
  ; Play sound in local folder
  SoundPlay A_ScriptDir "\deltaMedia\reload.wav"
  TrayTip "Reloaded", "Delta 2", 4 	; Traytip: https://www.autohotkey.com/docs/v2/lib/TrayTip.htm
  ; Run timer that reloads after 1 second
  SetTimer Reload, 1000
}

; Delta + Backslash to open this script in VSCode using scriptdir and make the window active
F20 & \::{
  Run "C:\Program Files\Microsoft VS Code\Code.exe " . A_ScriptDir
  ; WinWaitActive "Code.exe"
  ; WinActivate "Code.exe"
}

; Delta + Right Square Bracket to open scriptdir in explorer:
F20 & ]::{
  Run "explorer.exe " . A_ScriptDir
}



TraySetIcon A_ScriptDir . "\deltaMedia\delta.ico", 1 ,false ; Set tray icon to delta.ico https://www.autohotkey.com/docs/v2/lib/TraySetIcon.ht