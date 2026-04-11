import { from } from "./index.js";

const ir = from("posts").grid(3).card().toIR();

console.log(JSON.stringify(ir, null, 2));

