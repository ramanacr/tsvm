import { value } from "./module.ts";

export type Pair = {
  left: number;
  right: number;
};

class Box {
  value: number;
}

let enabled: boolean = true;
var label: string = "runtime";

if (enabled && value !== null) {
  for (let i = 0; i < 3; i += 1) {
    label = label + "." + i;
  }
} else {
  throw "disabled";
}

