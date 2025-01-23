import json
import csv
import os
import requests
from dotenv import load_dotenv

# Global configuration
load_dotenv()
OLLAMA_URL = os.getenv("OLLAMA_URL", "http://localhost:11434/api/chat")


def generate_wikibase_prompt(value, language):
    """
    Generate a structured prompt for Ollama to convert property values.
    """
    json_schema = {
        "type": "object",
        "properties": {"sentence": {"type": "string"}, "question": {"type": "string"}},
        "required": ["sentence", "question"],
    }

    prompt = f"""
    I want to convert Wikidata to synthetic data by using the triplets in the wikibase to proper sentences. 
    Given the Wikibase property "{value}", please generate an appropriate sentence and question in {language}.:
    
    Example for citizenship:
    - Sentence: "{{}} is citizen of {{}}."
    - Question: "{{}} was a citizen of what country?"
    
    Return as JSON.
    """

    return {
        "model": "phi4",
        "messages": [{"role": "user", "content": prompt}],
        "format": json_schema,
        "stream": False,
        "options": {"timeout": 60, "temperature": 0},
    }


def call_ollama(payload):
    """
    Make API call to Ollama with given payload.

    Args:
        payload (dict): Request payload for Ollama

    Returns:
        tuple: (sentence, question) or (None, None) if error
    """
    try:
        response = requests.post(OLLAMA_URL, json=payload)
        response.raise_for_status()
        result = response.json()

        if "message" in result and "content" in result["message"]:
            parsed_content = json.loads(result["message"]["content"])
            return parsed_content["sentence"], parsed_content["question"]

    except Exception as e:
        print(f"Ollama API error: {e}")

    return None, None


# Function to load JSON from a file
def load_json(file_path):
    with open(file_path, "r", encoding="utf-8") as file:
        return json.load(file)


def get_language_from_filename(filename):
    if "-en-" in filename:
        return "English"
    elif "-nl-" in filename:
        return "Dutch"
    else:
        return "Unknown"


# Function to convert JSON data to CSV format and save it to a file
def json_to_csv(json_data, output_file):
    language = get_language_from_filename(output_file)
    # Open the output CSV file in write mode
    with open(output_file, "w", newline="", encoding="utf-8") as csvfile:
        # Create a CSV writer object
        csvwriter = csv.writer(csvfile, delimiter=";")

        # Write the header row to the CSV file
        csvwriter.writerow(["key", "value", "sentence", "question"])

        # Iterate over each binding in the JSON data
        for binding in json_data["results"]["bindings"]:
            # Extract the values of interest
            property_value = binding["property"]["value"]
            stripped_key = property_value.replace("http://www.wikidata.org/entity/", "")
            property_label_value = binding["propertyLabel"]["value"]

            payload = generate_wikibase_prompt(property_label_value, language)
            sentence, question = call_ollama(payload)
            # Write a row to the CSV file
            print(f"{stripped_key}: {property_label_value}, {sentence}, {question}")
            csvwriter.writerow([stripped_key, property_label_value, sentence, question])


# Main execution
if __name__ == "__main__":
    # Specify the path to your JSON file
    json_file_path = "./wikidata-en-properties.json"
    # Load JSON data from the specified file
    json_data = load_json(json_file_path)
    # Convert and save the JSON data as a CSV file
    output_csv_file = "./wikidata-en-properties.csv"
    json_to_csv(json_data, output_csv_file)

    print(f"CSV file '{output_csv_file}' has been created.")

    # Specify the path to your JSON file
    json_file_path = "./wikidata-nl-properties.json"
    # Load JSON data from the specified file
    json_data = load_json(json_file_path)
    # Convert and save the JSON data as a CSV file
    output_csv_file = "./wikidata-nl-properties.csv"
    json_to_csv(json_data, output_csv_file)

    print(f"CSV file '{output_csv_file}' has been created.")
