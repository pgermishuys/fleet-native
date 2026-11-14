"""Real X11 input through XTest, for the spike's --demo on Linux: `click X Y` or `type TEXT`."""
import ctypes
import sys
import time

x11 = ctypes.cdll.LoadLibrary("libX11.so.6")
xtst = ctypes.cdll.LoadLibrary("libXtst.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XStringToKeysym.restype = ctypes.c_ulong
x11.XDefaultRootWindow.restype = ctypes.c_ulong
dpy = ctypes.c_void_p(x11.XOpenDisplay(None))


def key(sym):
    code = x11.XKeysymToKeycode(dpy, ctypes.c_ulong(x11.XStringToKeysym(sym.encode())))
    xtst.XTestFakeKeyEvent(dpy, code, True, 0)
    xtst.XTestFakeKeyEvent(dpy, code, False, 0)
    x11.XFlush(dpy)
    time.sleep(0.03)


if sys.argv[1] == "click":
    xtst.XTestFakeMotionEvent(dpy, -1, int(sys.argv[2]), int(sys.argv[3]), 0)
    x11.XFlush(dpy)
    time.sleep(0.15)
    xtst.XTestFakeButtonEvent(dpy, 1, True, 0)
    xtst.XTestFakeButtonEvent(dpy, 1, False, 0)
    x11.XFlush(dpy)
    # With no window manager, nothing gives the clicked window the keyboard; do what one would:
    # focus the top-level window under the pointer.
    root = x11.XDefaultRootWindow(dpy)
    ret_root, child = ctypes.c_ulong(), ctypes.c_ulong()
    i = ctypes.c_int()
    u = ctypes.c_uint()
    x11.XQueryPointer(dpy, ctypes.c_ulong(root), ctypes.byref(ret_root), ctypes.byref(child),
                      ctypes.byref(i), ctypes.byref(i), ctypes.byref(i), ctypes.byref(i), ctypes.byref(u))
    if child.value:
        x11.XSetInputFocus(dpy, child, 2, 0)  # RevertToParent, CurrentTime
        x11.XFlush(dpy)
else:
    for ch in sys.argv[2]:
        key(ch)
time.sleep(0.2)
