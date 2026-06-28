/** A tiny GitHub API client. Every call goes through request(), which adds the
 *  auth header, checks the status, and parses JSON. */

export class GitHub {
  private token: string;

  constructor(token: string) {
    this.token = token;
  }

  private async request(path: string): Promise<unknown> {
    const res = await fetch(`https://api.github.com${path}`, {
      headers: { Authorization: `Bearer ${this.token}` },
    });
    if (!res.ok) throw new Error(`GitHub ${res.status}`);
    return res.json();
  }

  async getUser(login: string): Promise<unknown> {
    return this.request(`/users/${login}`);
  }

  async getRepo(owner: string, name: string): Promise<unknown> {
    return this.request(`/repos/${owner}/${name}`);
  }
}
