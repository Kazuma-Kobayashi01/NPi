// Loads PDF.js (Apache-2.0, vendor/pdfjs/LICENSE) as a module and hands it to the classic scripts:
// `window.OMP_PDFJS`, announced by the `omp:pdfjs` event. The release CSP allows only same-origin scripts.
import * as pdfjs from "./pdf.min.mjs";
pdfjs.GlobalWorkerOptions.workerSrc = new URL("./pdf.worker.min.mjs", import.meta.url).href;
window.OMP_PDFJS = pdfjs;
window.dispatchEvent(new Event("omp:pdfjs"));
