/// 
Basic dictionary creation and access
///

let person = {
    "name": "Alice",
    "age": 30,
    "city": "New York"
}

print("Name:", person["name"])
print("Age:", person["age"])
print("City:", person["city"])

// Dictionary length
print("Number of keys:", len(person))

// Update a value
mut config = {
    "debug": True,
    "verbose": False
}

print("Initial debug mode:", config["debug"])
config["debug"] = False
print("Updated debug mode:", config["debug"])