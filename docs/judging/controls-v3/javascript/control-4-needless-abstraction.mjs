class Money {
  constructor(cents) {
    this.cents = cents;
  }

  minus(other) {
    return new Money(this.cents - other.cents);
  }

  plus(other) {
    return new Money(this.cents + other.cents);
  }
}

/** @abstract */
class FeePolicy {
  /** @param {Money} amount @returns {Money} */
  fee(amount) {
    throw new Error(`${this.constructor.name}.fee is not implemented`);
  }
}

class CardFeePolicy extends FeePolicy {
  fee(amount) {
    return new Money(Math.floor((amount.cents * 29 + 500) / 1000) + 30);
  }
}

class BankFeePolicy extends FeePolicy {
  fee(amount) {
    return new Money(25);
  }
}

class CashFeePolicy extends FeePolicy {
  fee(amount) {
    return new Money(0);
  }
}

class FeePolicyRegistry {
  #policies = new Map();

  register(kind, policy) {
    this.#policies.set(kind, policy);
    return this;
  }

  resolve(kind) {
    const policy = this.#policies.get(kind);
    if (policy === undefined) {
      throw new Error(`unknown kind: ${kind}`);
    }
    return policy;
  }
}

const REGISTRY = new FeePolicyRegistry()
  .register("card", new CardFeePolicy())
  .register("bank", new BankFeePolicy())
  .register("cash", new CashFeePolicy());

class Record {
  constructor(account, kind, amount) {
    [this.account, this.kind, this.amount] = [account, kind, amount];
  }
}

class RecordParser {
  #text;

  constructor(text) {
    this.#text = text;
  }

  parse() {
    return this.#text
      .split("\n")
      .map((line, index) => [index + 1, line])
      .filter(([, line]) => RecordParser.#keep(line))
      .map(([number, line]) => RecordParser.#parseLine(number, line));
  }

  static #keep(line) {
    return Boolean(line.trim()) && !line.trim().startsWith("#");
  }

  static #parseLine(number, line) {
    const fields = line.trim().split(",");
    if (fields.length !== 4) {
      throw new Error(`line ${number}: expected 4 fields, got ${fields.length}`);
    }
    return new Record(
      fields[1].trim(),
      fields[2].trim(),
      new Money(Number.parseInt(fields[3], 10)),
    );
  }
}

class BalanceAggregator {
  #registry;
  #totals = new Map();

  constructor(registry) {
    this.#registry = registry;
  }

  accept(record) {
    const fee = this.#registry.resolve(record.kind).fee(record.amount);
    const current = this.#totals.get(record.account) ?? new Money(0);
    this.#totals.set(record.account, current.plus(record.amount.minus(fee)));
    return this;
  }

  result() {
    return new Map([...this.#totals].map(([account, money]) => [account, money.cents]));
  }
}

class Renderer {
  render(totals) {
    return [...totals.keys()]
      .sort()
      .map((account) => `${account}  ${Renderer.#money(totals.get(account))}`);
  }

  static #money(cents) {
    const magnitude = Math.abs(cents);
    const dollars = `${Math.floor(magnitude / 100)}.${String(magnitude % 100).padStart(2, "0")}`;
    return cents < 0 ? `(${dollars})` : dollars;
  }
}

export function parseRecords(text) {
  return new RecordParser(text)
    .parse()
    .map((r) => ({ account: r.account, kind: r.kind, amountCents: r.amount.cents }));
}

export function feeCents(kind, amountCents) {
  return REGISTRY.resolve(kind).fee(new Money(amountCents)).cents;
}

export function balances(records) {
  const aggregator = new BalanceAggregator(REGISTRY);
  for (const { account, kind, amountCents } of records) {
    aggregator.accept(new Record(account, kind, new Money(amountCents)));
  }
  return aggregator.result();
}

export function render(totals) {
  return new Renderer().render(totals);
}
