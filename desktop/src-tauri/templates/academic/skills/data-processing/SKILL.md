---
name: data-processing
description: Process research data reproducibly - raw data stays untouched, outputs and the steps that made them go to data/processed.
---

# Data processing

1. Read from `data/raw/`; never modify it. Copy or derive into `data/processed/`.
2. Save the script or exact steps beside the output (`data/processed/<name>.py` or `<name>.md`) so the result can be rerun.
3. Record in a short header: source file, date, what was changed (filtering, units, joins), and row counts before and after.
4. State units and missing-value handling explicitly. Do not fill in missing values with guesses.
5. When a result feeds a comparison in `artifacts/`, cite the processed file it came from.
