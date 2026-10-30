"""Functionality to match phone numbers in a piece of text"""

# Based on original Java code:
#     java/src/com/google/i18n/phonenumbers/PhoneNumberMatch.java
#     java/src/com/google/i18n/phonenumbers/PhoneNumberMatcher.java
#   Copyright (C) 2011 The Libphonenumber Authors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
from .util import UnicodeMixin, unicod, prnt

try:
    from .data import _ALT_NUMBER_FORMATS
except ImportError:  # pragma no cover
    # Before the generated code exists, the data/ directory is empty.
    # The generation process imports this module, creating a circular
    # dependency.  The hack below works around this.
    import os
    import sys
    if os.path.basename(sys.argv[0]) in ("buildmetadatafromxml.py", "buildprefixdata.py"):
        prnt("Failed to import generated data (but OK as during autogeneration)", file=sys.stderr)
        _ALT_NUMBER_FORMATS = {}
    else:
        raise


class Leniency(object):
    """Leniency when finding potential phone numbers in text segments.

    The levels here are ordered in increasing strictness."""
    # Phone numbers accepted are possible (i.e. is_possible_number(number)) but
    # not necessarily valid (is_valid_number(number)).
    POSSIBLE = 0
    # Phone numbers accepted are both possible (is_possible_number(number))
    # and valid (is_valid_number(PhoneNumber)). Numbers written in national
    # format must have their national-prefix present if it is usually written
    # for a number of this type.
    VALID = 1
    # Phone numbers accepted are valid (i.e. is_valid_number(number)) and are
    # grouped in a possible way for this locale. For example, a US number
    # written as "65 02 53 00 00" and "650253 0000" are not accepted at this
    # leniency level, whereas "650 253 0000", "650 2530000" or "6502530000"
    # are.
    # Numbers with more than one '/' symbol in the national significant number
    # are also dropped at this level.
    #
    # Warning: This level might result in lower coverage especially for
    # regions outside of country code "+1". If you are not sure about which
    # level to use, email the discussion group
    # libphonenumber-discuss@googlegroups.com.
    STRICT_GROUPING = 2
    # Phone numbers accepted are valid (i.e. is_valid_number(number)) and are
    # grouped in the same way that we would have formatted it, or as a single
    # block. For example, a US number written as "650 2530000" is not accepted
    # at this leniency level, whereas "650 253 0000" or "6502530000" are.
    # Numbers with more than one '/' symbol are also dropped at this level.
    # Warning: This level might result in lower coverage especially for
    # regions outside of country code "+1". If you are not sure about which
    # level to use, email the discussion group
    # libphonenumber-discuss@googlegroups.com.
    EXACT_GROUPING = 3



class PhoneNumberMatcher(object):
    """A stateful class that finds and extracts telephone numbers from text.

    Vanity numbers (phone numbers using alphabetic digits such as '1-800-SIX-FLAGS' are
    not found.

    This class is not thread-safe.
    """

    def __init__(self, text, region,
                 leniency=Leniency.VALID, max_tries=65535,
                 min_candidate_length=1):
        """Creates a new instance.

        Arguments:
        text -- The character sequence that we will search, None for no text.
        country -- The country to assume for phone numbers not written in
              international format (with a leading plus, or with the
              international dialing prefix of the specified region). May be
              None or "ZZ" if only numbers with a leading plus should be
              considered.
        leniency -- The leniency to use when evaluating candidate phone
              numbers.
        max_tries -- The maximum number of invalid numbers to try before
              giving up on the text.  This is to cover degenerate cases where
              the text has a lot of false positives in it. Must be >= 0.
        min_candidate_length -- The minimum length of a candidate phone number.
              Can be used to quickly skip candidates that are too short to be valid,
              depending on your use-case needs.
        """
        raise NotImplementedError("not implemented in this workspace")

    def has_next(self):
        """Indicates whether there is another match available"""
        raise NotImplementedError("not implemented in this workspace")

    def next(self):
        """Return the next match; raises Exception if no next match available"""
        raise NotImplementedError("not implemented in this workspace")

    def __iter__(self):
        raise NotImplementedError("not implemented in this workspace")


class PhoneNumberMatch(UnicodeMixin):
    """The immutable match of a phone number within a piece of text.

    Matches may be found using the find() method of PhoneNumberMatcher.

    A match consists of the phone number (in .number) as well as the .start
    and .end offsets of the corresponding subsequence of the searched
    text. Use .raw_string to obtain a copy of the matched subsequence.

    The following annotated example clarifies the relationship between the
    searched text, the match offsets, and the parsed number:

    >>> text = "Call me at +1 425 882-8080 for details."
    >>> country = "US"
    >>> import phonenumbers
    >>> matcher = phonenumbers.PhoneNumberMatcher(text, country)
    >>> matcher.has_next()
    True
    >>> m = matcher.next()  # Find the first phone number match
    >>> m.raw_string # contains the phone number as it appears in the text.
    "+1 425 882-8080"
    >>> (m.start, m.end)  # define the range of the matched subsequence.
    (11, 26)
    >>> text[m.start, m.end]
    "+1 425 882-8080"
    >>> phonenumberutil.parse("+1 425 882-8080", "US") == m.number
    True
    """
    def __init__(self, start, raw_string, numobj):
        if start < 0:
            raise Exception("Start index not >= 0")
        if raw_string is None or numobj is None:
            raise Exception("Invalid argument")
        # The start index into the text.
        self.start = start
        # The raw substring matched.
        self.raw_string = raw_string
        self.end = self.start + len(raw_string)
        # The matched phone number.
        self.number = numobj

    def __eq__(self, other):
        if not isinstance(other, PhoneNumberMatch):
            return False
        return (self.start == other.start and
                self.raw_string == other.raw_string and
                self.end == other.end and
                self.number == other.number)

    def __ne__(self, other):
        return not self.__eq__(other)

    def __repr__(self):
        return (unicod("PhoneNumberMatch(start=%r, raw_string=%r, numobj=%r)") %
                (self.start,
                 self.raw_string,
                 self.number))

    def __unicode__(self):
        return unicod("PhoneNumberMatch [%s,%s) %s") % (self.start, self.end, self.raw_string)

