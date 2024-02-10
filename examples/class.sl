(class animal
	(let colour "red")
	(fun print ()
		(dump colour)
	)
	(fun set-colour (c)
		(set colour c)
	)
)

(let cat (new animal))
(print cat)
(set-colour cat "green")
(print cat)
