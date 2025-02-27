import os
import pandas as pd


def convert_csvs_to_parquet(
    input_folder: str, output_folder: str, chunksize: int = 100_000
):
    """Converts all CSV files in a folder to Parquet format using chunked reading."""
    os.makedirs(output_folder, exist_ok=True)  # Ensure output folder exists

    for filename in os.listdir(input_folder):
        if filename.endswith(".csv"):
            csv_path = os.path.join(input_folder, filename)
            parquet_path = os.path.join(
                output_folder, filename.replace(".csv", ".parquet")
            )

            print(f"Converting {csv_path} to {parquet_path}...")

            first_chunk = True
            for chunk in pd.read_csv(csv_path, chunksize=chunksize):
                chunk.to_parquet(
                    parquet_path,
                    engine="pyarrow",
                    index=False,
                    compression="snappy",
                    append=not first_chunk,
                )
                first_chunk = False

            print(f"Done: {parquet_path}")


# Example usage
convert_csvs_to_parquet("../output/nl", "../output/nl")
