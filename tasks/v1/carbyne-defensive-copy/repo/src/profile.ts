/** A user profile. Internal state is never exposed by reference: accessors
 *  return copies/immutable views so callers cannot mutate the profile's internals. */

export class Profile {
  private _name: string;
  private _roles: string[];

  constructor(name: string, roles: string[]) {
    this._name = name;
    this._roles = [...roles];
  }

  get name(): string {
    return this._name;
  }
}
