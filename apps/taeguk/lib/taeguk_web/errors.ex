defmodule TaegukWeb.ErrorHTML do
  def render(template, _assigns), do: Phoenix.Controller.status_message_from_template(template)
end

defmodule TaegukWeb.ErrorJSON do
  def render(template, _assigns),
    do: %{error: Phoenix.Controller.status_message_from_template(template)}
end
