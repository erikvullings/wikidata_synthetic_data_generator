#!/usr/bin/env python3
import csv
import sys
import re

# Use stdin/stdout for streaming large files
input_file = sys.stdin
output_file = sys.stdout

# CSV reader/writer setup
csv_reader = csv.reader(input_file, delimiter=';', quotechar='"')
csv_writer = csv.writer(output_file, delimiter=';', quotechar='"', quoting=csv.QUOTE_MINIMAL)

# Write header for the output
csv_writer.writerow(['label', 'aliases'])

# Pattern for extracting aliases
pattern = r'(.*) is also known as (.*)\.'

# Process each row
for row in csv_reader:
    if not row:  # Skip empty rows
        continue
        
    # Extract the original label from first column
    original_label = row[0]
    
    # Default: no aliases
    aliases = ""
    
    # Check if we have a second column with sentence data
    if len(row) > 1 and row[1]:
        sentence = row[1]
        match = re.search(pattern, sentence)
        
        if match:
            # If the sentence matches our pattern, use the entity from sentence as label
            # and the aliases part as aliases
            entity = match.group(1).strip()
            aliases = match.group(2).strip()
            
            # If you want to use the original label from first column and 
            # just extract aliases, comment the line below
            # original_label = entity
    
    # Write the output row
    csv_writer.writerow([original_label, aliases])
