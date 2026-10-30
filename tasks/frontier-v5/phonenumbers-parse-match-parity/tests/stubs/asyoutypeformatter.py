"""A formatter which formats phone numbers as they are entered.

An AsYouTypeFormatter can be created by invoking
AsYouTypeFormatter(region_code). After that digits can be added by invoking
input_digit() on the formatter instance, and the partially formatted phone
number will be returned each time a digit is added. clear() should be invoked
before a new number needs to be formatted.

See the unit tests for more details on how the formatter is to be used.
"""

# Based on original Java code:
#     java/src/com/google/i18n/phonenumbers/AsYouTypeFormatter.java
#   Copyright (C) 2009-2011 The Libphonenumber Authors
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


class AsYouTypeFormatter(object):

    def __init__(self, region_code):
        """Gets an AsYouTypeFormatter for the specific region.

        Arguments:
        region_code -- The region where the phone number is being entered

        Return an AsYouTypeFormatter object, which could be used to format
        phone numbers in the specific region "as you type"
        """
        raise NotImplementedError("not implemented in this workspace")

    def clear(self):
        """Clears the internal state of the formatter, so it can be reused."""
        raise NotImplementedError("not implemented in this workspace")

    def input_digit(self, next_char, remember_position=False):
        """Formats a phone number on-the-fly as each digit is entered.

        If remember_position is set, remembers the position where next_char is
        inserted, so that it can be retrieved later by using
        get_remembered_position. The remembered position will be automatically
        adjusted if additional formatting characters are later
        inserted/removed in front of next_char.

        Arguments:

        next_char -- The most recently entered digit of a phone
              number. Formatting characters are allowed, but as soon as they
              are encountered this method formats the number as entered and
              not "as you type" anymore. Full width digits and Arabic-indic
              digits are allowed, and will be shown as they are.
        remember_position -- Whether to track the position where next_char is
              inserted.

        Returns the partially formatted phone number.
        """
        raise NotImplementedError("not implemented in this workspace")

    def get_remembered_position(self):
        """Returns the current position in the partially formatted phone
        number of the character which was previously passed in as the
        parameter of input_digit(remember_position=True)."""
        raise NotImplementedError("not implemented in this workspace")

