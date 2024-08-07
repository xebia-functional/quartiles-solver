# Dictionaries

Custom word list generated from http://app.aspell.net/create using
[SCOWL](http://wordlist.aspell.net) with parameters:

* `diacritic`: strip
* `max_size`: 60
* `max_variant`: 3
* `special`: <none>
* `spelling`: US

Using Git Commit From: `Mon Dec 7 20:14:35 2020 -0500 [5ef55f9]`

```
Copyright 2000-2019 by Kevin Atkinson

  Permission to use, copy, modify, distribute and sell these word
  lists, the associated scripts, the output created from the scripts,
  and its documentation for any purpose is hereby granted without fee,
  provided that the above copyright notice appears in all copies and
  that both that copyright notice and this permission notice appear in
  supporting documentation. Kevin Atkinson makes no representations
  about the suitability of this array for any purpose. It is provided
  "as is" without express or implied warranty.
```

Word list was post-processed to:

1. Remove proper nouns (really, anything with a majuscule character other than
  the lone word I)
2. Remove entries with non-alphanumeric characters
3. Remove entries without vowels (with the exception of CWM and CRWTH)
4. Remove commonly censored words
5. Remove words longer than 16 letters (because they can't appear in Quartiles)

Notes:

* No attempt was made to remove other abbreviations.
* No attempt was made to to preserve common interjections (HMM, PFFT, …).
* No attempt was made to achieve equivalence or parity with the official
  Quartiles dictionary.
