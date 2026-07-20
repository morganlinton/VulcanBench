/* G15 negative twin  -  __restrct__ is a typo and not a valid keyword alias.
 * gcc rejects: unknown type name '__restrct__'
 */
int foo(__restrct__ int *p) { return *p; }
