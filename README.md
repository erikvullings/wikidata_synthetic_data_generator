# WikiData synthetic data generation

For LLM training, generate synthetic data from Wikidata entities and properties.

This repository provides code that extract entities and their properties from Wikidata and saves them to CSV format. For example:

```csv
George Washington is also known as American Fabius.
George Washington is also known as Father of the United States.
George Washington is also known as The American Fabius.
George Washington is a president of the United States from 1789 to 1797.
George Washington is occupied as politician.
George Washington is citizen of Kingdom of Great Britain.
"George Washington was born on February 22, 1732."
"George Washington died on December 14, 1799."
George W. Bush is also known as Bush Jr..
George W. Bush is also known as Dubya.
George W. Bush is also known as GWB.
George W. Bush is also known as Bush 43.
George W. Bush is also known as President George W. Bush.
George W. Bush is also known as George Bush.
George W. Bush is also known as President Bush.
George W. Bush is also known as Bush.
"George W. Bush is also known as Bush, George W.."
George W. Bush is a President of the United States from 2001 to 2009.
George W. Bush is occupied as politician.
George W. Bush is citizen of United States of America.
"George W. Bush was born on July 06, 1946."
Augusto Pinochet is also known as Augusto Pinochet Ugarte.
Augusto Pinochet is a dictator of Chile from 1973 to 1990.
Augusto Pinochet is occupied as military personnel.
Augusto Pinochet is citizen of Chile.
"Augusto Pinochet was born on November 25, 1915."
"Augusto Pinochet died on December 10, 2006."
Diego Velázquez is a Spanish painter (1599-1660).
Diego Velázquez is occupied as painter.
Diego Velázquez is citizen of Spain.
"Diego Velázquez was born on June 06, 1599."
"Diego Velázquez died on August 06, 1660."
Charles Baudelaire is also known as Baudelaire.
Charles Baudelaire is also known as Charles Pierre Baudelaire-Dufaÿs.
Charles Baudelaire is also known as Charles Pierre Baudelaire.
Charles Baudelaire is a French poet and critic (1821–1867).
Charles Baudelaire works in the area of poetry.
Charles Baudelaire is occupied as poet.
Charles Baudelaire is citizen of France.
"Charles Baudelaire was born on April 09, 1821."
"Charles Baudelaire died on August 31, 1867."
```

## Prerequisites

Download the latest WikiData entry, e.g. you can use aria2 to download it more efficiently (in 2024, it is 130Gb):

```ps1
# In case you need to install it
choco install aria2
aria2c.exe -x 16 https://dumps.wikimedia.org/wikidatawiki/entities/latest-all.json.gz
```

Unzip the data to `latest-all.json` using 7zip or some efficient unzipper that shows progress, as 1.5Tb takes some time to unzip.

## Run

Using English as the main language:

```bash
cargo run --release -- /d/data/wikidata/latest-all.json -l en -e person -o ./output
```

Alternatively, specify the language, e.g. using Dutch:

```bash
cargo run --release -- /d/data/wikidata/latest-all.json -l nl -e person -o ./output
```

Alternatively, on Windows:

```ps1
cargo run --release D:\data\wikidata\latest-all.json -l en -e person -o ./output
```

## Queries

[Script source](https://github.com/kermitt2/grisp/blob/master/scripts/wikipedia-resources.sh).

To get the language-specific labels of the Wikibase properties `Pxxx`, download the properties in JSON format using a SPARQL query. For example, replace `<TWO_LETTER_LANGUAGE_CODE>` with `en` for English or `nl` for Dutch:

```bash
 wget "https://query.wikidata.org/sparql?format=json&query=SELECT%20%3Fproperty%20%3FpropertyLabel%20WHERE%20%7B%0A%20%20%20%20%3Fproperty%20a%20wikibase%3AProperty%20.%0A%20%20%20%20SERVICE%20wikibase%3Alabel%20%7B%0A%20%20%20%20%20%20bd%3AserviceParam%20wikibase%3Alanguage%20%22<TWO_LETTER_LANGUAGE_CODE>%22%20.%0A%20%20%20%7D%0A%20%7D%0A%0A" -O wikidata-<TWO_LETTER_LANGUAGE_CODE>-properties.json
 ```

To get the latest page properties, language links and actual articles:

```PS1
 aria2c.exe -x 16 https://dumps.wikimedia.org/<TWO_LETTER_LANGUAGE_CODE>wiki/latest/<TWO_LETTER_LANGUAGE_CODE>wiki-latest-pages-articles-multistream.xml.bz2
 aria2c.exe -x 16 https://dumps.wikimedia.org/<TWO_LETTER_LANGUAGE_CODE>wiki/latest/<TWO_LETTER_LANGUAGE_CODE>wiki-latest-page_props.sql.gz
 aria2c.exe -x 16 https://dumps.wikimedia.org/<TWO_LETTER_LANGUAGE_CODE>wiki/latest/<TWO_LETTER_LANGUAGE_CODE>wiki-latest-langlinks.sql.gz
 ```

- Latest pages contains the Wikimedia text represented as XML, and can be converted into an HTML page. The crate [`parse_wiki_text`](https://crates.io/crates/parse_wiki_text) parses the Wikimedia text to an AST (representation as Rust objects).
- Latest page properties has an index (`pp_page`) that refers to the index of the page block, e.g. Albert Speer has index 1, and is also the first page in the latest pages xml. In addition, it contains the Wikibase identifier, e.g. [Q60045](https://www.wikidata.org/wiki/Q60045), which refers to Albert Speer too, but now using a Q identifier.
- Loop through the language array to create translation files for each target language `(<PAGE_ID>, <TO_LANGUAGE_CODE>, <NAMED_ENTITY_IN_TO_LANGUAGE>)`, e.g. if the NER entity in a German text is Andre Agassi, it refers to `(2,'de','Andre Agassi')` or `(2,'awa','आन्द्रे अगासी')` in Awadhi, so entry 2 in the page properties and pages articles represents the tennis player `Andre Agassi`. In this case, the names are the same, but this is not always the case. For example, `(378,'en','Der Blaue Reiter')` in English becomes `(378,'az','Göy atlı')` in Azerbaijan.

So if you want to perform NER on a German text, and you want to display the results in Dutch, you take the `nlwiki-latest-langlinks.sql.gz` and extract all triplets that link the `de` language code to a page id and translation. Next, you compare a NER entity from the German text to the translated version. If there is a match, you can lookup the Wikibase identifier from the page props, and get the information text from pages articles.