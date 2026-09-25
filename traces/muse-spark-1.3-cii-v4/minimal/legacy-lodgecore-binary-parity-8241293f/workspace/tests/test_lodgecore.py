"""Golden regression tests for lodgecore.py.

Each case feeds a fixed command session to the engine CLI and asserts
the exact byte output. The expected outputs were captured from the
retired LodgeCore engine; the replacement must reproduce them
byte-for-byte (its actual behavior, which the spec only approximates,
is the contract).
"""

import subprocess
import sys
import unittest
from pathlib import Path

ENGINE = [sys.executable, str(Path(__file__).resolve().parent.parent
                              / "lodgecore.py")]


def run(session: str) -> str:
    proc = subprocess.run(ENGINE, input=session.encode(), capture_output=True)
    assert proc.returncode == 0, proc.stderr.decode()
    return proc.stdout.decode()


BERTH_TIE_ASC = """P a 4
P c 4
O R0 10
O R1 10
B a R0
A a
B c R0
A c
Q R0
"""

BERTH_TIE_ASC_OUT = """OK 1
OK 2
OK 1
OK 2
OK 1
IN R0
OK 1
IN R0
RM R0 a c
X 2 2 2 2 0 0 0
"""

BERTH_TIE_DESC = """P a 4
P c 4
O R0 10
O R1 10
O R2 10
O R3 10
W
W
W
W
W
W
B a R0
A a
B c R0
A c
Q R0
"""

BERTH_TIE_DESC_OUT = """OK 1
OK 2
OK 1
OK 2
OK 3
OK 4
AIRED R0 0
AIRED R1 0
AIRED R2 0
AIRED R3 0
AIRED R0 0
AIRED R1 0
OK 1
IN R0
OK 1
IN R0
RM R0 c a
X 2 4 2 2 0 6 0
"""

SETTLE_RESTAFF = """P p0 1
P p1 1
O R0 9
O R1 6
O R2 8
B p1 R0
W
S
B p0 R0
W
W
S
W
W
A p1
S
"""

SETTLE_RESTAFF_OUT = """OK 1
OK 2
OK 1
OK 2
OK 3
OK 1
AIRED R0 0
SETTLED 0
RM R0
RM R1
RM R2
OK 2
AIRED R0 1
MV p0 R2
AIRED R1 0
SETTLED 1
RM R0 p0
RM R1
RM R2
AIRED R1 0
AIRED R2 0
IN R0
SETTLED 2
RM R0
RM R1 p1 p0
RM R2
X 2 3 2 2 0 5 3
"""

REV_TOGGLE = """P a 4
P b 1
O R0 10
O R1 10
B a R0
A a
B b R0
A b
W
W
S
W
W
W
"""

REV_TOGGLE_OUT = """OK 1
OK 2
OK 1
OK 2
OK 1
IN R0
OK 1
IN R0
AIRED R0 2
MV a R1
MV b R1
AIRED R1 2
MV a R0
MV b R0
SETTLED 2
RM R0 a b
RM R1
AIRED R1 0
AIRED R0 2
MV b R1
MV a R1
AIRED R1 2
MV b R0
MV a R0
X 2 2 2 2 0 5 1
"""

QUEUED_BOOKING = """P c 4
P d 1
O R0 10
O R1 10
W
B c R0
B d R0
S
W
"""

QUEUED_BOOKING_OUT = """OK 1
OK 2
OK 1
OK 2
AIRED R0 0
OK 1
OK 2
SETTLED 0
RM R0
RM R1
AIRED R0 2
MV d R1
MV c R1
X 2 2 2 2 0 2 1
"""


MIXED_RECORD_RANKS_ABOVE_EQUAL = """P b 2
P a 1
O R 10
B a R
B b R
A a
A b
G a
B a R
A a
G a
B a R
A a
L a
B a R
A a
S
"""

MIXED_RECORD_RANKS_ABOVE_EQUAL_OUT = """OK 1
OK 2
OK 1
OK 1
OK 2
IN R
IN R
OUT R
OK 1
IN R
OUT R
OK 1
IN R
OUT R
OK 1
IN R
SETTLED 2
RM R a b
X 2 1 5 5 3 0 1
"""

# The berth tie direction looks back two seasons: R is rested twice
# before the first settling (so its previous-season count is 2) and
# not at all after, yet the tie still falls to later registration
# first at the second settling.
TWO_SEASON_TIE = """P a 1
P b 1
O R 10
O T 10
O X 10
B a R
B b R
A a
A b
W
W
W
W
W
S
G a
G b
P c 1
P d 1
B c X
B d X
A c
A d
S
"""

TWO_SEASON_TIE_OUT = """OK 1
OK 2
OK 1
OK 2
OK 3
OK 1
OK 2
IN R
IN R
AIRED R 2
MV a T
MV b X
AIRED T 1
MV a R
AIRED X 1
MV b T
AIRED R 1
MV a X
AIRED T 1
MV b R
SETTLED 2
RM R b a
RM T
RM X
OUT R
OUT R
OK 3
OK 4
OK 1
OK 2
IN X
IN X
SETTLED 2
RM R d c
RM T
RM X
X 4 3 4 4 2 5 2
"""


class TestLodgecore(unittest.TestCase):
    def test_berth_tie_registration_order(self):
        self.assertEqual(run(BERTH_TIE_ASC), BERTH_TIE_ASC_OUT)

    def test_mixed_departure_record_ranks_above_equal(self):
        self.assertEqual(run(MIXED_RECORD_RANKS_ABOVE_EQUAL),
                         MIXED_RECORD_RANKS_ABOVE_EQUAL_OUT)

    def test_two_season_tie_direction(self):
        self.assertEqual(run(TWO_SEASON_TIE), TWO_SEASON_TIE_OUT)

    def test_berth_tie_flips_after_rests(self):
        self.assertEqual(run(BERTH_TIE_DESC), BERTH_TIE_DESC_OUT)

    def test_settle_restaffs_and_orders(self):
        self.assertEqual(run(SETTLE_RESTAFF), SETTLE_RESTAFF_OUT)

    def test_rota_reversal_direction(self):
        self.assertEqual(run(REV_TOGGLE), REV_TOGGLE_OUT)

    def test_queued_booking_berths_and_reverses(self):
        self.assertEqual(run(QUEUED_BOOKING), QUEUED_BOOKING_OUT)


if __name__ == "__main__":
    unittest.main()
