# Convert wikibase property keys and values to sentences and questions using Python and Ollama

## Installation instructions

```bash
python -m venv .venv
source .venv/bin/activate  # On Windows use `.\.venv\Scripts\activate`
pip install -r requirements.txt
```

Also, set the `OLLAMA_API_KEY` environment variable to your Ollama API key. You can use `example.env` as an example of `.env` for that.

## Retrieving the property data

Original query for each language, where {i} needs to be replaced by the two-letter language code, e.g. `en` for English and `nl` for Dutch:

```bash
wget "https://query.wikidata.org/sparql?format=json&query=SELECT%20%3Fproperty%20%3FpropertyLabel%20WHERE%20%7B%0A%20%20%20%20%3Fproperty%20a%20wikibase%3AProperty%20.%0A%20%20%20%20SERVICE%20wikibase%3Alabel%20%7B%0A%20%20%20%20%20%20bd%3AserviceParam%20wikibase%3Alanguage%20%22en%22%20.%0A%20%20%20%7D%0A%20%7D%0A%0A" -O wikidata-properties.json`
```

```sql
SELECT ?property ?propertyLabel ?datatype WHERE {
    ?property a wikibase:Property .
    ?property wikibase:propertyType ?datatype .
    SERVICE wikibase:label {
        bd:serviceParam wikibase:language "${i}" .
    }
}
```

## Improved query

```bash
https://query.wikidata.org/sparql?format=json&query=SELECT%20%3Fproperty%20%3FpropertyID%20%3FpropertyLabel%20%3Fdatatype%20%3Fpath%20WHERE%20%7B%0A%20%20%20%20%3Fproperty%20a%20wikibase%3AProperty%20.%0A%20%20%20%20%3Fproperty%20wikibase%3ApropertyType%20%3Fdatatype%20.%0A%20%20%20%20BIND(STRAFTER(STR(%3Fproperty)%2C%20%22http%3A%2F%2Fwww.wikidata.org%2Fentity%2F%22)%20AS%20%3FpropertyID)%0A%20%20%20%20VALUES%20(%3Fdatatype%20%3Fpath)%20%7B%0A%20%20%20%20%20%20(wikibase%3AWikibaseItem%20%22mainsnak.datavalue.value.id%22)%0A%20%20%20%20%20%20(wikibase%3ATime%20%22mainsnak.datavalue.value.time%22)%0A%20%20%20%20%20%20(wikibase%3AString%20%22mainsnak.datavalue.value%22)%0A%20%20%20%20%20%20(wikibase%3AExternalId%20%22mainsnak.datavalue.value%22)%0A%20%20%20%20%20%20(wikibase%3AQuantity%20%22mainsnak.datavalue.value.amount%22)%0A%20%20%20%20%20%20(wikibase%3AUrl%20%22mainsnak.datavalue.value%22)%0A%20%20%20%20%20%20(wikibase%3AGlobeCoordinate%20%22mainsnak.datavalue.value%22)%0A%20%20%20%20%20%20(wikibase%3ACommonsMedia%20%22mainsnak.datavalue.value%22)%0A%20%20%20%20%20%20(wikibase%3AMonolingualText%20%22mainsnak.datavalue.value.text%22)%0A%20%20%20%20%7D%0A%20%20%20%20SERVICE%20wikibase%3Alabel%20%7B%0A%20%20%20%20%20%20bd%3AserviceParam%20wikibase%3Alanguage%20%22en%22%20.%0A%20%20%20%7D%0A%7D
```

Representing the following query:

```sql
SELECT ?property ?propertyID ?propertyLabel ?datatype ?path WHERE {
    ?property a wikibase:Property .
    ?property wikibase:propertyType ?datatype .
    BIND(STRAFTER(STR(?property), "http://www.wikidata.org/entity/") AS ?propertyID)
    VALUES (?datatype ?path) {
        (wikibase:WikibaseItem "mainsnak.datavalue.value.id")
        (wikibase:Time "mainsnak.datavalue.value.time")
        (wikibase:String "mainsnak.datavalue.value")
        (wikibase:ExternalId "mainsnak.datavalue.value")
        (wikibase:Quantity "mainsnak.datavalue.value.amount")
        (wikibase:Url "mainsnak.datavalue.value")
        (wikibase:GlobeCoordinate "mainsnak.datavalue.value")
        (wikibase:CommonsMedia "mainsnak.datavalue.value")
        (wikibase:MonolingualText "mainsnak.datavalue.value.text")
    }
    SERVICE wikibase:label {
        bd:serviceParam wikibase:language "${i}" .
    }
}
```

For example, P31 is the property for "instance of":

```json
{
  "datatype": {
    "type": "uri",
    "value": "http://wikiba.se/ontology#WikibaseItem"
  },
  "path": {
    "type": "literal",
    "value": "mainsnak.datavalue.value.id"
  },
  "property": {
    "type": "uri",
    "value": "http://www.wikidata.org/entity/P31"
  },
  "propertyID": {
    "type": "literal",
    "value": "P31"
  },
  "propertyLabel": {
    "xml:lang": "en",
    "type": "literal",
    "value": "instance of"
  }
}
```
