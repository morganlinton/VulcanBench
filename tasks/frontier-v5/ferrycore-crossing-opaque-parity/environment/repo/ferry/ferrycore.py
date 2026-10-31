#!/usr/bin/env python3
"""FerryCore crossing log: Python replacement for the retired engine.

Implements docs/SPEC.md. Usage: python ferry/ferrycore.py < session.txt
"""

from __future__ import annotations

import re
import sys

NAME = re.compile(r"[A-Za-z0-9]{1,8}\Z")
NUMBER = re.compile(r"[0-9]{1,2}\Z")
SEPARATORS = re.compile(r"[ \t\r\v\f]+")
DECK_AXLES = 12

# command word -> argument kinds
COMMANDS = {
    "K": ("name", "number"),
    "P": ("name", "number"),
    "Q": ("name", "name"),
    "X": ("name",),
    "M": ("name", "name"),
    "W": ("name",),
    "S": (),
    "T": (),
    "N": (),
}

TRAILER_FIELDS = ("K", "P", "Q", "X", "M", "S", "T", "N")


class Rejected(Exception):
    pass


class CrossingLog:
    def __init__(self, out=None) -> None:
        self.out = out if out is not None else sys.stdout
        self.cars: dict[str, int] = {}  # name -> axles, in registration order
        self.slips: dict[str, int] = {}  # name -> room, in opening order
        self.lines: dict[str, list[str]] = {}  # slip -> cars, front first
        self.waiting_at: dict[str, str] = {}  # car -> slip
        self.crossings: dict[str, int] = {}
        self.ferry_at: str | None = None
        self.accepted = dict.fromkeys(TRAILER_FIELDS, 0)

    def write(self, line: str) -> None:
        self.out.write(line + "\n")

    # ------------------------------------------------------------ parsing
    def handle(self, raw: str) -> None:
        text = raw.strip(" \t\r\v\f\n")
        if not text:
            return
        word, *args = SEPARATORS.split(text)
        try:
            kinds = COMMANDS.get(word)
            if kinds is None or len(args) != len(kinds):
                raise Rejected("FORM")
            for kind, token in zip(kinds, args):
                pattern = NAME if kind == "name" else NUMBER
                if not pattern.match(token):
                    raise Rejected("FORM")
            getattr(self, "do_" + word)(*args)
        except Rejected as reject:
            self.write(f"NAY {reject}")
            return
        if word in self.accepted:
            self.accepted[word] += 1

    def car(self, name: str) -> str:
        if name not in self.cars:
            raise Rejected("STRAY")
        return name

    def slip(self, name: str) -> str:
        if name not in self.slips:
            raise Rejected("WILD")
        return name

    # ----------------------------------------------------------- commands
    def do_K(self, name: str, number: str) -> None:
        axles = int(number)
        if not 1 <= axles <= 9:
            raise Rejected("WIDE")
        if name in self.cars:
            raise Rejected("TWICE")
        self.cars[name] = axles
        self.crossings[name] = 0
        self.write(f"OK {len(self.cars)}")

    def do_P(self, name: str, number: str) -> None:
        room = int(number)
        if not 1 <= room <= 20:
            raise Rejected("WIDE")
        if name in self.slips:
            raise Rejected("TWICE")
        self.slips[name] = room
        self.lines[name] = []
        if self.ferry_at is None:
            self.ferry_at = name
        self.write(f"OK {len(self.slips)}")

    def do_Q(self, car: str, slip: str) -> None:
        car, slip = self.car(car), self.slip(slip)
        if car in self.waiting_at:
            raise Rejected("TWICE")
        self.join(car, slip)
        self.write("OK")

    def do_X(self, car: str) -> None:
        car = self.car(car)
        if car not in self.waiting_at:
            raise Rejected("IDLE")
        self.lines[self.waiting_at.pop(car)].remove(car)
        self.write("OK")

    def do_M(self, car: str, slip: str) -> None:
        car, slip = self.car(car), self.slip(slip)
        if car not in self.waiting_at:
            raise Rejected("IDLE")
        if self.waiting_at[car] == slip:
            raise Rejected("TWICE")
        if len(self.lines[slip]) >= self.slips[slip]:
            raise Rejected("CRAM")
        self.lines[self.waiting_at.pop(car)].remove(car)
        self.join(car, slip)
        self.write("OK")

    def join(self, car: str, slip: str) -> None:
        if len(self.lines[slip]) >= self.slips[slip]:
            raise Rejected("CRAM")
        self.lines[slip].append(car)
        self.waiting_at[car] = slip

    def do_W(self, slip: str) -> None:
        line = self.lines[self.slip(slip)]
        self.write(f"LINE {slip} {len(line)}")
        for car in line:
            self.write(f"C {car}")

    def do_S(self) -> None:
        if self.ferry_at is None:
            raise Rejected("WILD")
        here = self.ferry_at
        room_left = DECK_AXLES
        boarded = []
        for car in self.lines[here]:
            if self.cars[car] <= room_left:
                boarded.append(car)
                room_left -= self.cars[car]
        for car in boarded:
            self.lines[here].remove(car)
            del self.waiting_at[car]
            self.crossings[car] += 1
        route = list(self.slips)
        self.ferry_at = route[(route.index(here) + 1) % len(route)]
        self.write(f"SAIL {here} {len(boarded)}")
        for car in boarded:
            self.write(f"B {car}")
        self.write(f"DOCK {self.ferry_at}")

    def do_T(self) -> None:
        order = list(self.cars)
        crossed = [car for car in order if self.crossings[car]]
        crossed.sort(key=lambda car: (-self.crossings[car], order.index(car)))
        self.write(f"TOLL {len(crossed)}")
        for car in crossed:
            self.write(f"C {car}")

    def do_N(self) -> None:
        order = list(self.cars)
        listing = sorted(order, key=lambda car: (-self.cars[car], order.index(car)))
        self.write(f"NIGHT {len(listing)}")
        for car in listing:
            self.write(f"C {car}")

    def finish(self) -> None:
        self.write("END " + " ".join(str(self.accepted[f]) for f in TRAILER_FIELDS))


def main() -> None:
    log = CrossingLog()
    for line in sys.stdin.read().split("\n"):
        log.handle(line)
    log.finish()


if __name__ == "__main__":
    main()
