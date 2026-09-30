#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root=join(dirname(fileURLToPath(import.meta.url)),"../..");
const files=[
  "mobile-android/app/src/main/res/values/strings.xml",
  "mobile-android/app/src/main/res/values-fa/strings.xml",
];
function names(path){
  const xml=readFileSync(join(root,path),"utf8");
  return [...xml.matchAll(/<string\s+name="([^"]+)"/g)].map(m=>m[1]).sort();
}
const en=names(files[0]), fa=names(files[1]);
const missingFa=en.filter(x=>!fa.includes(x));
const missingEn=fa.filter(x=>!en.includes(x));
if(missingFa.length||missingEn.length){
  if(missingFa.length) console.error("Android i18n missing in values-fa:",missingFa.join(", "));
  if(missingEn.length) console.error("Android i18n missing in values:",missingEn.join(", "));
  process.exit(1);
}
console.log(`Android i18n parity passed: ${en.length} resources in values and values-fa.`);
