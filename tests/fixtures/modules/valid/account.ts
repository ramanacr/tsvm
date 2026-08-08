export interface Account {
  id: number;
  balance: number;
}

export function credit(account: Account, amount: number): number {
  account.balance += amount;
  return account.balance;
}
