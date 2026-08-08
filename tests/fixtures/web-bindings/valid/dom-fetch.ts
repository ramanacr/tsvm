function domSetText(selector: string, text: string): undefined {
  return undefined;
}

function domText(selector: string): string {
  return "";
}

function fetchText(url: string): string {
  return "";
}

domSetText("#app", fetchText("/message.txt"));
console.log(domText("#app"));
