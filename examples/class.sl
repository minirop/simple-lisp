(class animal
	(let colour "red")
	(fun print ()
		(dump "LOL")
		(dump colour)
		(dump "LALA")
	)
)

(let cat (new animal))
(dump cat)
(print cat)
