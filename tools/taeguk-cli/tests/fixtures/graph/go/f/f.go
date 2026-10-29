package f

import "plugin"

func Load(p string) error {
	_, err := plugin.Open(p)
	return err
}
