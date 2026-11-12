/*
 * vbscroll: post scroll-wheel events at the pointer, inside a macOS guest.
 *
 *     vbscroll DY [DX]        lines; positive DY scrolls up (content moves down)
 *     vbscroll --check        exit 0 if this process may post events
 *     vbscroll --prompt       ask macOS to list the responsible app under
 *                             Privacy & Security > Accessibility
 *
 * Virtualization.framework's VNC server does not deliver scroll-wheel
 * events, so the ComputerUse macOS suite scrolls through this helper, run by
 * Tart's guest agent (docs/DECISIONS.md, 2026-11-11). Posting events needs
 * the guest agent to hold Accessibility permission, which the base image
 * recipe grants.
 *
 * Build on the host:  clang -O2 -framework ApplicationServices vbscroll.c -o vbscroll
 */
#include <ApplicationServices/ApplicationServices.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc >= 2 && strcmp(argv[1], "--check") == 0) {
        return AXIsProcessTrusted() ? 0 : 1;
    }
    if (argc >= 2 && strcmp(argv[1], "--prompt") == 0) {
        const void *keys[] = {kAXTrustedCheckOptionPrompt};
        const void *vals[] = {kCFBooleanTrue};
        CFDictionaryRef opts = CFDictionaryCreate(NULL, keys, vals, 1,
            &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
        Boolean ok = AXIsProcessTrustedWithOptions(opts);
        CFRelease(opts);
        return ok ? 0 : 1;
    }
    if (argc < 2) {
        fprintf(stderr, "usage: vbscroll DY [DX] | --check | --prompt\n");
        return 2;
    }
    int dy = atoi(argv[1]);
    int dx = argc >= 3 ? atoi(argv[2]) : 0;
    if (!AXIsProcessTrusted()) {
        fprintf(stderr, "vbscroll: not allowed to post events (Accessibility)\n");
        return 3;
    }
    /* One event per line, so apps that accelerate scrolling see a steady wheel. */
    int n = abs(dy) > abs(dx) ? abs(dy) : abs(dx);
    for (int i = 0; i < n; i++) {
        int32_t sy = (i < abs(dy)) ? (dy > 0 ? 1 : -1) : 0;
        int32_t sx = (i < abs(dx)) ? (dx > 0 ? 1 : -1) : 0;
        CGEventRef e = CGEventCreateScrollWheelEvent(NULL, kCGScrollEventUnitLine, 2, sy, sx);
        if (!e) return 4;
        CGEventPost(kCGHIDEventTap, e);
        CFRelease(e);
        usleep(12000);
    }
    return 0;
}
