### Yield Max
This is a rust tool that analyzes a 300mm wafer map text file provided by the user as a text file input and locates the highest yielding 200mm region on the wafer.

The text file provided by the user contains an ASCII representation of the 300mm wafer with each die represented by a single character in a 17x17 grid. A sample of the file format is shown below:

.....1111111.....
...XX111111XXX...
..X11X111X1X111..
.XXX1111111X111X.
.XXXX1111XX11X1X.
XXXXX1X111111XX1X
XXXXXX1111111X1XX
X1X111X1X1X11X1XX
XXX1XXXXXXX111XXX
1XX1XXX1X11X11XX1
XXXXXXXXX1111X11X
.11XXXXX11111X11.
.X1XXX1X11111XXX.
..1XX1XXXXX1111..
...XXXXXXXXXXX...
....X11111X1X....
.......XXXX......

The '.' character represents a non-present die.
The 'X' character represents a defect die.
The '1' character represents a good die.

The 200mm wafer shape is an 11x11 grid. An example is shown below:
...OOOOO...
..OOOOOOO..
.OOOOOOOOO.
OOOOOOOOOOO
OOOOOOOOOOO
OOOOOOOOOOO
OOOOOOOOOOO
.OOOOOOOOO.
.OOOOOOOOO.
..OOOOOOO..
....OOO....

The '.' character represents a non-present die.
The 'O' character represents a present die in the 200mm wafer.

The goal of this program is to find the 200mm region of the 300mm wafer to maximize the number of good die in from the 300mm wafer overlayed with the 'O' character from the 200mm mask. The program should report the number of good die in the 200mm region and generate a new version of the 300mm text file that marks all die in the optimal 200mm region with the 'Z' character.
