(class animal
	(let colour "red")
	(fun print ()
		(dump colour)
	)
	(fun set-colour (c)
		(set colour c)
	)
)

(fun print (c)
	(dump c)
)

(let cat (new animal))
(print cat) /* prints "string: red" */
(set-colour cat "green")
(print cat) /* prints "string: green" */
(print "hello")
(print 42)
