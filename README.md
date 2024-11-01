# Py-PoloDB Python Bindings

## Overview
This repository contains the Python bindings for the [PoloDB](https://www.polodb.org) project. These bindings allow Python code to interface seamlessly with the PoloDB core functionalities written in Rust.

## Installation 
```bash
python3.9 -m pip install polodb-python
```

## Usage : 
```python
>>> from polodb import PoloDB 
>>> db  = PoloDB("db")
>>> db
<polodb.core.PoloDB object at 0x1001d6a00>
>>> col = db.collection('my-collection')
>>> col
<polodb.core.Collection object at 0x100244d00>
>>> data = [{"foo":"bar", "titi":"kpkp"}, {"lol":"out", "foo":"bar"}]
>>> col.insert_many(data)
{0: '6725102e0c6d6f91b9df53bd', 1: '6725102e0c6d6f91b9df53be'}
>>> col.len()
2
>>> col.find({"lol":"out"})
[{'lol': 'out', 'foo': 'bar', '_id': '6725102e0c6d6f91b9df53be'}]
```

### Current methods supported for collection

 * delete_one
 * delete_many
 * find
 * find_one
 * insert_many
 * insert_one
 * len
 * name
 * update_many
 * update_one
 * aggregate