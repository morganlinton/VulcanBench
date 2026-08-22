# Discriminated unions reject members whose tag field is a bare `None` annotation

A discriminated-union member can tag itself with `Literal[None]`, but the
natural spelling — annotating the field as plain `None` — is rejected at
model build time:

```python
class A(BaseModel):
    field: Literal['A'] = 'A'

class B(BaseModel):
    field: None = None

class Model(BaseModel):
    a_or_b: Annotated[A | B, Field(discriminator='field')]
# PydanticUserError: Model 'B' needs field 'field' to be of type `Literal`
```

(Upstream report: pydantic/pydantic#13660.)

Expected: a bare `None` annotation works as a discriminator tag exactly like
`Literal[None]` does today.

- The model builds; `{'field': None}` routes to `B` and `{'field': 'A'}`
  still routes to `A`.
- The JSON schema's `discriminator.mapping` maps the `'null'` key to the
  `None`-tagged member's schema, alongside the literal tags.
- Existing behavior is unchanged: string-literal unions route and reject
  unknown tags as today, their JSON schema mapping is unchanged, and
  `Literal[None]` tags keep working.
