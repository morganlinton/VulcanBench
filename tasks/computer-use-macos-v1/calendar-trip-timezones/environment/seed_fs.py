"""calendar-trip-timezones seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

Four PDFs in ~/Downloads/Trip. Times are local to each airport or hotel.
Today is Monday 9 March 2026; the US moved to daylight time on 8 March and
Portugal and Germany move on 29 March, so in the trip week Lisbon is UTC+0
(4 hours ahead of Boston, not 5) and Munich UTC+1.
  AX 218   Boston Mon 16 Mar 21:30 -> Lisbon Tue 17 Mar 09:05
           (01:30Z to 09:05Z 17 Mar; 21:30 to 5:05 next morning in Boston time)
  Casa Alfama Suites, check-out Fri 20 Mar by 11:00 Lisbon (11:00Z)
  AW 1173  Lisbon Fri 20 Mar 12:40 -> Munich 16:25 (12:40Z to 15:25Z)
  AX 216   Boston Mon 9 Mar, CANCELLED (schedule change, rebooked on AX 218)
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

L = S.Line


def booking(brand: str, sub: str, lines: list[tuple[str, bool]], foot: list[str], stamp: str | None = None) -> bytes:
    it: list[object] = [L(72, 730, brand, 20, bold=True), L(72, 710, sub, 9, gray=0.3), S.Rule(72, 696, 540, 696)]
    y = 670.0
    for text, bold in lines:
        if text:
            it.append(L(72, y, text, 11 if bold else 10, bold=bold))
        y -= 17
    y -= 20
    for f in foot:
        it.append(L(72, y, f, 9, gray=0.25))
        y -= 13
    if stamp:
        it.append(S.Stamp(320, 430, stamp))
    return S.pdf([it])


def documents() -> dict[str, bytes]:
    d = "Downloads/Trip/"
    return {
        d + "AX218 Boston-Lisbon e-ticket.pdf": booking(
            "Atlantica Air", "E-ticket and itinerary receipt  |  Booking ref QX7K2M  |  Passenger: JORDAN LEE",
            [("Flight AX 218  Boston (BOS) to Lisbon (LIS)", True),
             ("Departs: Monday 16 March 2026, 21:30  Boston Logan, Terminal E", False),
             ("Arrives: Tuesday 17 March 2026, 09:05  Lisbon Humberto Delgado, Terminal 1", False),
             ("Duration 6h 35m, overnight, nonstop", False), ("", False),
             ("Seat 23A  |  Economy  |  1 checked bag", False)],
            ["All times are local to the airport.", "Online check-in opens 24 hours before departure."]),
        d + "Casa Alfama Suites - reservation.pdf": booking(
            "Casa Alfama Suites", "Rua de Sao Miguel 12, 1100-544 Lisboa, Portugal  |  Reservation 88-4410",
            [("Guest: Jordan Lee", True), ("Check-in: Tuesday 17 March 2026, from 15:00", False),
             ("Check-out: Friday 20 March 2026, by 11:00", False), ("3 nights, Deluxe double, breakfast included", False)],
            ["Times are Lisbon local time.", "Early check-in on request, subject to availability."]),
        d + "AW1173 Lisbon-Munich e-ticket.pdf": booking(
            "Alpenlink Airways", "Electronic ticket  |  Booking code 4MRT9P  |  Passenger: LEE/JORDAN",
            [("Flight AW 1173  Lisbon (LIS) to Munich (MUC)", True),
             ("Departs: Friday 20 March 2026, 12:40  Lisbon, Terminal 1", False),
             ("Arrives: Friday 20 March 2026, 16:25  Munich, Terminal 2", False),
             ("Duration 2h 45m, nonstop", False), ("", False), ("Seat 8C  |  Economy Light  |  Cabin bag only", False)],
            ["Times shown are local times at the airports.", "Check-in opens 24 hours before departure."]),
        d + "AX216 Boston-Lisbon e-ticket (old).pdf": booking(
            "Atlantica Air", "E-ticket and itinerary receipt  |  Booking ref QX7K2M  |  Passenger: JORDAN LEE",
            [("Flight AX 216  Boston (BOS) to Lisbon (LIS)", True),
             ("Departs: Monday 9 March 2026, 18:10  Boston Logan, Terminal E", False),
             ("Arrives: Tuesday 10 March 2026, 05:45  Lisbon, Terminal 1", False)],
            ["This flight was cancelled after a schedule change. You have been rebooked on AX 218."],
            stamp="CANCELLED"),
    }


STAMP = "2026-03-06 18:30"


def main(root: Path) -> None:
    home = root / "Users" / "admin"
    for rel, data in documents().items():
        S.put(home / rel, STAMP, data)
    S.stamp_dir(home / "Downloads" / "Trip", STAMP)
    for rel in documents():
        print(rel, (home / rel).stat().st_size)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
