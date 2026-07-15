use std::time::Duration;

pub struct Config<Id> {
	// the nodes identity - who am I?
	pub me: Id,
	
	//how many peers a message is forwarded to in a given round (derived from the published works on HyPerView)
	pub fanout: usize,

	//how long a node waits between gossip rounds
	pub round_interval: Duration, 
	
	// How many rounds a payload keeps being reshared before it is finished distributing 
	pub max_rounds: u32,
}

impl<Id> Config<Id> { 
	//config gossip defaults from HyPerView Article Specs
	pub fn new(me: Id) -> Self {
		Config {
			me, 
			fanout: 3,
			round_interval: Duration::from_secs(1),
			max_rounds: 5,
		}
	}
}

