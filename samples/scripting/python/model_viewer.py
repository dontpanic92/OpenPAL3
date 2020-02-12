from gi.repository import Application

class App(Application.Application):
    def __init__(self):
        super().__init__()
    
    def do_on_updated(self, delta_time):
        print("delta_time " + str(delta_time))

a = App()
a.initialize()
a.run()
