package b

import (
	"fmt"

	"example.com/fx/c"
)

func B() int { return len(fmt.Sprint(c.C())) }
