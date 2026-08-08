import { Account, credit } from "./account.ts";

const account: Account = {
  id: 1,
  balance: 100
};

console.log(credit(account, 50));
