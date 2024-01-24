# 𝚫2
This is my personal AHK v2.0 script, feel free to make it your own.
## About
Caps Lock is a *[literally](https://en.wikipedia.org/wiki/Caps_Lock#/media/File:Depressing_caps_lock.jpg)* mechanically outdated and ultimately rote keyboard key. And it's a pretty big key! That makes it ripe for modification and permutation. 

I have rebound the Caps Lock key on my keyboards and redefined it as [Delta ](https://en.wiktionary.org/wiki/%CE%94). This is the main modifier key for most of my non-app-specific bindings. It's simply the F20 key, in windows [many applications](https://stefansundin.github.io/altdrag/) will accept F keys alongside other keypresses to bind actions.

I do most of my work on a Macbook and this script is simply an attempt to recreate some very fast / comfy window movement and app specific keybind shorthands I've created in [Keyboard Maestro](https://www.keyboardmaestro.com/main/)  with the added benefits that deploying this script can be fast since [AHK](https://github.com/AutoHotkey/AutoHotkey/releases) is freely available and the .exe is under 2 megs. 
### ToDo
- Add more bindings. This script is largely incomplete due to mostly using the Macbook. As I use my PC more this script will inevitably grow.
- Make deployable via a [DuckyScript™️](https://docs.hak5.org/hak5-usb-rubber-ducky/duckyscript-tm-quick-reference) that will clone this repo, deploy itself and intercept Caps Lock keypresses as F20, so you'll never be too far from your own rapid workflows.
	-  Self destruct keybind to delete the portable install and shutdown so you can use your commands on a friend's system. 
-  Intercept the Caps-Lock key and output F20 or simply also trigger by Caps-Lock. Whichever is most reliable and flexible.
## On rotary encoders:
I like having a rotary encoder knob and find they're better on the right side of the board for
- Faster / easier access when your hand is on a mouse
- Ability to hit standard modifier keys (ctrl,win,alt,shift) with left hand at the same time making multifunction knob workflows possible / ergonomic.
Because of this I've adopted the convention that turning the knob right (clockwise) is "down" since the portion of the knob that you touch moves downward with your right hand. (Think of which way your hand moves if you grabbed and turned a steering wheel with your right hand).
On my boards:
- Encoder left == F16 
- Encoder right == F17
## On mousing:
As much as I like navigating interfaces with the keyboard alone, using a mouse is a necessary evil for many tasks; be it browsing the web or navigating file browsers. That's why you'll find that you can use most of the core keybinds with just your left hand, allowing you to leave your right hand free to use a mouse.
